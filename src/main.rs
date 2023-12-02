use actix_web::{get, web, App, HttpRequest, HttpServer, Responder};
use std::io::{stdin, stdout, Read, Write, Cursor};
use std::path::{Path, PathBuf};
use flate2::read::GzDecoder;
use std::sync::{Arc, Mutex};
use serde::Deserialize;
use toml::de::from_str;
use actix_files as af;
use tokio_postgres::{Config, NoTls};
use home::home_dir;
use std::fs::File;
use text_io::read;
use tar::Archive;
use toml::Value;
use log::{info, error};
use std::fs;
use std;
use tokio;

mod endpoints;
const WEB_DIR_ARCHIVE: &'static [u8] = include_bytes!("../web.tar.gz");
const WEB_VERSION: &'static str = include_str!("../web/version.txt");

#[derive(Debug, Deserialize)]
struct DBConfig {
    address: String,
    port: u16,
    user: String,
    passw: String
}

#[derive(Debug, Deserialize)]
struct GridConfig {
    x_size: i32,
    y_size: i32
}

#[derive(Debug, Deserialize)]
struct AppConfig {
    db: DBConfig,
    grid: GridConfig
}

fn read_config(file: PathBuf) -> Result<AppConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file)?;
    let config: AppConfig = from_str(&content)?;
    Ok(config)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let lib_dir = home_dir().expect("bruh you have no home directory").join("librarygrid");
    if std::fs::read_dir(&lib_dir).is_err() {
        first_time(lib_dir.clone());
    }
    
    if fs::metadata(lib_dir.clone().join("config.toml")).is_err() {
        config_helper(lib_dir.clone().join("config.toml"));
    }

    match check_ver(lib_dir.clone()) {
        Ok(()) => {},
        Err(e) => {
            error!("there was an error: {}", e);
            std::process::exit(1);
        }
    }

    let config = read_config(lib_dir.clone().join("config.toml")).expect("could not read config file!");
    let mut db_config = Config::new();
    db_config
        .user(config.db.user.as_str())
        .password(config.db.passw.as_str())
        .host(config.db.address.as_str())
        .port(config.db.port)
        .dbname("libdb");
    
    let (client, connection) = db_config
        .connect(NoTls)
        .await
        .expect("Failed to connect to the database");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Database connection error: {}", e);
        }
    });

    let db_client = Arc::new(Mutex::new(client));

    info!("server is starting up!");
    HttpServer::new(move || {
        App::new()
            .wrap(actix_web::middleware::Logger::default())
            .app_data(web::Data::new(db_client.clone()))
            .service(endpoints::booksearch)
            .service(af::Files::new("/", lib_dir.clone()).index_file("index.html"))
            .default_service(web::route().to(endpoints::notfound))
    }).bind("0.0.0.0:8080")?.run().await
}

fn first_time(lib_dir: PathBuf) {
    println!("--- welcome to the librarygrid software! ---");
    println!("it appears you're a new user. would you like to install the software? (installs to `{}`)", lib_dir.display());
    print!("(y/n) ");
    let yn: String = read!("{}\n");
    let fc = yn.chars().next().unwrap();
    if fc != 'y' && fc != 'Y' {
        println!("goodbye!");
        std::process::exit(0);
    }

    match std::fs::create_dir(&lib_dir) {
        Ok(_) => info!("directory created successfully!"),
        Err(e) => {
            error!("there was an error: {}", e);
            std::process::exit(1);
        }
    }
    
    match std::fs::create_dir(&lib_dir.join("web")) {
        Ok(_) => info!("web directory created successfully!"),
        Err(e) => {
            error!("there was an error: {}", e);
            std::process::exit(1);
        }
    }

    match extract_files(lib_dir) {
        Ok(()) => info!("web dir was extracted successfully!"),
        Err(e) => {
            error!("there was an error: {}", e);
            std::process::exit(1);
        }
    }
}

fn config_helper(config_file: PathBuf) {
    println!("--- config setup! ---");
    print!("postgres db address: ");
    let pda: String = read!("{}\n");
    print!("postgres db port: ");
    let pdp: i32 = read!("{}\n");
    print!("postgres user: ");
    let pu: String = read!("{}\n");
    print!("postgres passw: ");
    let pp: String = read!("{}\n");
    print!("grid x size: ");
    let gx: i32 = read!("{}\n");
    print!("grid y size: ");
    let gy: i32 = read!("{}\n");
    let tfc = format!("[db]\naddress = \"{}\"\nport = {}\nuser = \"{}\"\npassw = \"{}\"\n\n[grid]\nx_size = {}\ny_size = {}", pda, pdp, pu, pp, gx, gy);
    let mut file = match File::create(config_file.clone()) {
        Ok(file) => file,
        Err(err) => {
            error!("there was an error! {}", err);
            std::process::exit(1);
        }
    };

    match file.write_all(tfc.as_bytes()) {
        Ok(()) => {
            println!("config written successfully. please restart this program");
            std::process::exit(0);
        }
        Err(err) => {
            error!("there was an error! {}", err);
            std::process::exit(1);
        }
    }
}

fn check_ver(lib_dir: PathBuf) -> Result<(), std::io::Error> {
    let mut version_file = File::open(&lib_dir.join("web/version.txt"))?;
    let mut cur_version = String::new();
    version_file.read_to_string(&mut cur_version)?;
    info!("current version: {}", cur_version);
    if WEB_VERSION != cur_version {
        info!("down-/upgrading to web interface version: {}", WEB_VERSION);
        std::fs::remove_dir_all(&lib_dir.join("web"))?;
        std::fs::create_dir(&lib_dir.join("web"))?;
        extract_files(lib_dir.clone());
    }
    Ok(())
}

fn extract_files(output: PathBuf) -> Result<(), std::io::Error> {
    let cur = Cursor::new(WEB_DIR_ARCHIVE);
    let dec = GzDecoder::new(cur);
    let mut archive = Archive::new(dec);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path: PathBuf = entry.path()?.to_owned().to_path_buf();
        let full_path = output.join(Path::new("./web/")).join(&path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry.unpack(full_path)?;
    }
    Ok(())
}
