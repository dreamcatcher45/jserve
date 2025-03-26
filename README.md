# JServe 🚀

A **lightning-fast**, **RESTful JSON server** built with Rust ⚙️, designed for prototyping and mock APIs. Store and manage data effortlessly using a simple JSON file! 📁

![MIT License](https://img.shields.io/badge/License-MIT-green.svg)

## Features ✨

- 🌐 Full REST API support (GET/POST/PUT/DELETE)
- 🛠️ CRUD operations with JSON validation
- ⚡ Real-time persistence to JSON file
- 🔒 Thread-safe in-memory caching
- 🚦 Auto-create missing JSON files
- 📡 Dynamic endpoint discovery
- 🔧 Configurable port & file path
- 📡 Built with async Actix-Web framework


## Installation 📥

**Using Cargo**:
```bash
# Install directly from crates.io
cargo install jserve
```

**Pre-built Binaries** (Recommended):
1. Visit [GitHub Releases](https://github.com/dreamcatcher45/jserve/releases)
2. Download the appropriate executable for your OS:

### Windows 🪟
```bash
# Download (Run in PowerShell)
curl -LO https://github.com/dreamcatcher45/jserve/releases/latest/download/jserve-windows.exe

# Rename and make available system-wide (optional)
mv jserve-windows.exe jserve.exe
mkdir -p $HOME\bin
move .\jserve.exe $HOME\bin  # Add $HOME\bin to PATH
```

### Linux 🐧
```bash
# Download
curl -LO https://github.com/dreamcatcher45/jserve/releases/latest/download/jserve-linux

# Make executable and install
chmod +x jserve-linux
sudo mv jserve-linux /usr/local/bin/jserve
```

### macOS 🍎
```bash
# Download
curl -LO https://github.com/dreamcatcher45/jserve/releases/latest/download/jserve-macos

# Install globally
chmod +x jserve-macos
sudo mv jserve-macos /usr/local/bin/jserve
```

## Usage 🚀
```bash
# Start server with your JSON file
jserve -f db.json -p 3000

# Access from any directory after PATH setup!
```

**Path Configuration** 🌍:
- Windows: Add containing directory to `PATH` environment variable
- Unix/Mac: Use `/usr/local/bin` or add custom directory to `$PATH`

No Rust installation required! Just download and run 🎉


## Build from source 📦

**Prerequisites**: 
- Rust 1.65+ (install via [rustup](https://rustup.rs/))

```bash
# Clone repository
git clone https://github.com/dreamcatcher45/jserve.git
cd jserve

# Build and run (release mode recommended)
cargo build --release
./target/release/jserve -f db.json -p 3000
```

## Usage 🎮

**Sample Requests**:
```bash
# Get all posts
curl http://localhost:3000/posts

# Get specific post
curl http://localhost:3000/posts/1

# Create new post
curl -X POST -H "Content-Type: application/json" \
  -d '{"id":"4","title":"New Post"}' \
  http://localhost:3000/posts

# Update post
curl -X PUT -H "Content-Type: application/json" \
  -d '{"id":"1","title":"Updated Title"}' \
  http://localhost:3000/posts/1

# Delete post
curl -X DELETE http://localhost:3000/posts/1
```

**Automatic Endpoints**:
```
http://localhost:3000/{collection_name}
```
Based on your JSON file structure (e.g., `/posts`, `/activities` from sample db.json)

## Why JServe? 💡

- 🏎️ **Blazing Fast**: Built with Rust's performance and Actix-Web's async power
- 🧩 **Zero Dependencies**: Just a single binary and JSON file
- 🔄 **Real Persistence**: Changes saved instantly to disk
- 📊 **JSON Validation**: Strict schema checking for data integrity
- 🔐 **Concurrency Safe**: RWLock-protected data access
- 🐳 **Docker Ready**: Easy to containerize for deployments

## Contributing 🤝

We 💖 contributions! Here's how to help:

1. Fork the repository
2. Create feature branch (`git checkout -b feature/amazing`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing`)
5. Open Pull Request

**Development Setup**:
```bash
cargo run -- -f db.json  # Development mode
cargo test               # Run tests
```

Please follow Rust coding conventions and document complex logic.

## License 📜

MIT License - see [LICENSE](LICENSE) file for details
