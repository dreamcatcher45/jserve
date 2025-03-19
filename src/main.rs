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
use uuid::Uuid;

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
            .find(|item| {
                item.get("id").map(|v| v.as_str()) == Some(Some(id.as_str()))
            })
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
    let mut item_value = item.into_inner();
    
    if !item_value.is_object() {
        error!("Invalid JSON format for POST request");
        return Ok(HttpResponse::BadRequest().body("Expected JSON object"));
    }
    
    // Generate ID if missing
    let item_obj = item_value.as_object_mut().unwrap();
    if !item_obj.contains_key("id") {
        let new_id = Uuid::new_v4().to_string();
        item_obj.insert("id".to_string(), Value::String(new_id));
    }
    
    let item_id = item_obj["id"].as_str().unwrap().to_string();
    
    let duplicate = {
        let mut db = data.db.write().await;
        let items = db.entry(resource.clone()).or_default();
        if items.iter().any(|i| i.get("id").and_then(|v| v.as_str()) == Some(&item_id)) {
            true
        } else {
            items.push(Value::Object(item_obj.clone()));
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
                if let Some(index) = items.iter().position(|item| 
                    item.get("id").and_then(|v| v.as_str()) == Some(id.as_str())
                ) {
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
                if let Some(index) = items.iter().position(|item| 
                    item.get("id").and_then(|v| v.as_str()) == Some(id.as_str())
                ) {
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
    
    let db_value: Value = from_str(&content)
        .context("Failed to parse JSON")?;

    let mut db = Db::new();

    if let Value::Object(map) = db_value {
        for (resource, value) in map {
            match value {
                Value::Array(arr) => {
                    let mut validated_arr = Vec::new();
                    for item in arr {
                        if let Value::Object(obj) = item {
                            validated_arr.push(Value::Object(obj));
                        } else {
                            anyhow::bail!(
                                "Item in resource '{}' is not a JSON object",
                                resource
                            );
                        }
                    }
                    db.insert(resource, validated_arr);
                }
                Value::Object(obj) => {
                    db.insert(resource, vec![Value::Object(obj)]);
                }
                _ => {
                    anyhow::bail!(
                        "Resource '{}' is of invalid type - must be array or object",
                        resource
                    );
                }
            }
        }
    } else {
        anyhow::bail!("Top-level JSON must be an object");
    }

    Ok(db)
}

#[actix_web::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let args: Vec<String> = env::args().collect();
    let mut file_path = None;
    let mut port = 3000;

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

    fn print_help(program_name: &str) {
        println!("\nJServe - JSON REST server built in rust");
        println!("Version: {}\n", env!("CARGO_PKG_VERSION"));
        println!("Usage: {} [OPTIONS]", program_name);
        println!("\nOptions:");
        println!("  -f <FILE>\tJSON file to use as database (required)");
        println!("  -p <PORT>\tPort to listen on (default: 3000)");
        println!("  -v\t\tShow version information");
        println!("  -h\t\tShow this help message\n");
        println!("Website: https://github.com/yourusername/jserve");
        println!("GitHub: https://github.com/yourusername/jserve");
    }

    let file_path = file_path.unwrap_or_else(|| {
        eprintln!("Error: Missing required -f argument");
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

    let base_url = format!("http://localhost:{}", port);
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