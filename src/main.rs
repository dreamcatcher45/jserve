// src/main.rs
use actix_web::{
    delete, get, post, put,
    web::{self, Data, Json, Path},
    App, HttpResponse, HttpServer, Responder,
};
use anyhow::{Context, Result};
use log::error;
use serde_json::{Value, from_str, to_string_pretty};
use std::{
    collections::HashMap,
    path::Path as StdPath,
    sync::Arc,
};
use tokio::sync::RwLock;
use std::process;
use std::env;


type Db = HashMap<String, Vec<Value>>;

struct AppState {
    db: Arc<RwLock<Db>>,
    file_path: String,
}

#[derive(Debug)]
struct AppError(anyhow::Error);

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl actix_web::error::ResponseError for AppError {}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError(err)
    }
}

struct JsonFileStorage {
    state: Data<AppState>,
}

impl JsonFileStorage {
    fn new(state: Data<AppState>) -> Self {
        Self { state }
    }

    async fn save(&self) -> Result<()> {
        let db = self.state.db.read().await;
        let json_str = to_string_pretty(&*db)
            .context("Failed to serialize database to JSON")?;
        tokio::fs::write(&self.state.file_path, json_str)
            .await
            .context("Failed to write to JSON file")?;
        Ok(())
    }
}

#[get("/{resource}")]
async fn get_all(
    path: Path<String>,
    data: Data<AppState>,
) -> Result<impl Responder, AppError> {
    let resource = path.into_inner();
    let db = data.db.read().await;
    match db.get(&resource) {
        Some(items) => Ok(HttpResponse::Ok().json(items)),
        None => {
            error!("Resource not found: {}", resource);
            Ok(HttpResponse::NotFound().body("Resource not found"))
        }
    }
}

#[get("/{resource}/{id}")]
async fn get_one(
    path: web::Path<(String, String)>,
    data: Data<AppState>,
) -> Result<impl Responder, AppError> {
    let (resource, id) = path.into_inner();
    let db = data.db.read().await;
    match db.get(&resource) {
        Some(items) => items
            .iter()
            .find(|item| item["id"] == id)
            .map(|item| HttpResponse::Ok().json(item))
            .ok_or_else(|| {
                error!("Item not found: {}/{}", resource, id);
                AppError(anyhow::anyhow!("Item not found"))
            }),
        None => {
            error!("Resource not found: {}", resource);
            Ok(HttpResponse::NotFound().body("Resource not found"))
        }
    }
}

#[post("/{resource}")]
async fn create_item(
    path: Path<String>,
    item: Json<Value>,
    data: Data<AppState>,
    storage: web::Data<JsonFileStorage>,
) -> Result<impl Responder, AppError> {
    let resource = path.into_inner();
    
    // Changed from mut item_value to just item_value
    let item_value = item.into_inner();
    if !item_value.is_object() {
        error!("Invalid JSON format for POST request");
        return Ok(HttpResponse::BadRequest().body("Expected JSON object"));
    }

    // Extract ID as owned String
    let item_id = match item_value["id"].as_str() {
        Some(id) => id.to_string(),
        None => {
            error!("Missing 'id' field in item");
            return Ok(HttpResponse::BadRequest().body("Missing 'id' field"));
        }
    };

    let duplicate = {
        let mut db = data.db.write().await;
        let items = db.entry(resource.clone()).or_default();
        
        if items.iter().any(|i| i["id"] == item_id) {
            true
        } else {
            items.push(item_value);
            false
        }
    };

    if duplicate {
        error!("Duplicate ID: {}", item_id);
        return Ok(HttpResponse::Conflict().body("Duplicate ID"));
    }

    storage.save().await?;
    Ok(HttpResponse::Created().json(item_id))
}

#[put("/{resource}/{id}")]
async fn update_item(
    path: web::Path<(String, String)>,
    new_item: Json<Value>,
    data: Data<AppState>,
    storage: web::Data<JsonFileStorage>,
) -> Result<impl Responder, AppError> {
    let (resource, id) = path.into_inner();
    
    if !new_item.is_object() {
        error!("Invalid JSON format for PUT request");
        return Ok(HttpResponse::BadRequest().body("Expected JSON object"));
    }

    let result = {
        let mut db = data.db.write().await;
        match db.get_mut(&resource) {
            Some(items) => {
                if let Some(index) = items.iter().position(|item| item["id"] == id) {
                    items[index] = new_item.into_inner();
                    Ok(Some(index))
                } else {
                    Ok(None)
                }
            },
            None => Ok(None)
        }
    };

    match result {
        Ok(Some(_)) => {
            storage.save().await?;
            Ok(HttpResponse::Ok().json(id))
        },
        Ok(None) => {
            error!("Item not found: {}/{}", resource, id);
            Ok(HttpResponse::NotFound().body("Item not found"))
        },
        Err(e) => Err(e)
    }
}

#[delete("/{resource}/{id}")]
async fn delete_item(
    path: web::Path<(String, String)>,
    data: Data<AppState>,
    storage: web::Data<JsonFileStorage>,
) -> Result<impl Responder, AppError> {
    let (resource, id) = path.into_inner();

    let deleted = {
        let mut db = data.db.write().await;
        match db.get_mut(&resource) {
            Some(items) => {
                if let Some(index) = items.iter().position(|item| item["id"] == id) {
                    Some(items.remove(index))
                } else {
                    None
                }
            },
            None => None
        }
    };

    match deleted {
        Some(item) => {
            storage.save().await?;
            Ok(HttpResponse::Ok().json(item))
        },
        None => {
            error!("Item not found: {}/{}", resource, id);
            Ok(HttpResponse::NotFound().body("Item not found"))
        }
    }
}

async fn load_db(file_path: &str) -> Result<Db> {
    let content = tokio::fs::read_to_string(file_path)
        .await
        .context("Failed to read database file")?;
    let db: Db = from_str(&content)
        .context("Invalid JSON structure. Expected top-level object with arrays")?;
    
    for (resource, items) in &db {
        for (idx, item) in items.iter().enumerate() {
            if !item["id"].is_string() {
                anyhow::bail!(
                    "Invalid data in {} at index {}: Missing or invalid 'id' field",
                    resource,
                    idx
                );
            }
        }
    }
    Ok(db)
}

#[actix_web::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let args: Vec<String> = env::args().collect();
    let mut file_path = None;
    let mut port = 3000;

    // Handle help and version first
    for arg in &args {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help(&args[0]);
                process::exit(0);
            }
            "-v" | "--version" => {
                println!("jserve version {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            _ => {}
        }
    }

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-f" => {
                if i + 1 < args.len() {
                    file_path = Some(args[i+1].clone());
                    i += 2;
                } else {
                    eprintln!("Error: -f requires a filename");
                    process::exit(1);
                }
            },
            "-p" => {
                if i + 1 < args.len() {
                    port = args[i+1].parse()
                        .unwrap_or_else(|_| {
                            eprintln!("Invalid port number: {}", args[i+1]);
                            process::exit(1);
                        });
                    i += 2;
                } else {
                    eprintln!("Error: -p requires a port number");
                    process::exit(1);
                }
            },
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                process::exit(1);
            }
        }
    }

    // Add help text function
    fn print_help(_program_name: &str) {
        println!("\nJServe - JSON REST server built in rust");
        println!("Version: {}\n", env!("CARGO_PKG_VERSION"));
        println!("Options:");
        println!("  -f <FILE>    JSON file to use as database (required)");
        println!("  -p <PORT>    Port to listen on (default: 3000)");
        println!("  -v           Show version information");
        println!("  -h           Show this help message\n");
        println!("Website: https://dreamcatcher45.github.io/jserve");
        println!("GitHub:  https://github.com/dreamcatcher45/jserve\n");
    }

    let file_path = file_path.unwrap_or_else(|| {
        eprintln!("Error: Missing required -f argument. ");
        process::exit(1);
    });

    let path = StdPath::new(&file_path);
    
    if !path.exists() {
        tokio::fs::write(path, "{}")
            .await
            .context("Failed to create initial JSON file")?;
    }

    let db = load_db(&file_path).await?;
    let state = Data::new(AppState {
        db: Arc::new(RwLock::new(db)),
        file_path: file_path.clone(),
    });

    let base_url = format!("http://127.0.0.1:{}", port);
    println!("\nAvailable endpoints:");
    {
        let db = state.db.read().await;
        for resource in db.keys() {
            println!("- {}/{}", base_url, resource);
        }
    }
    println!("\nServer running at {}", base_url);

    let storage = web::Data::new(JsonFileStorage::new(state.clone()));

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .app_data(storage.clone())
            .service(get_all)
            .service(get_one)
            .service(create_item)
            .service(update_item)
            .service(delete_item)
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await?;

    Ok(())
}