use flate2::read::GzDecoder;
use std::path::{Path, PathBuf};
use lazy_static::lazy_static;
use std::io::{stdin, stdout, Write, Read, Cursor, self};
use home::home_dir;
use std::fs::File;
use std::process;
use tar::Archive;
use std;

const WEB_DIR_ARCHIVE: &'static [u8] = include_bytes!("../web.tar.gz");
const WEB_VERSION: &'static str = include_str!("../web/version.txt");

fn main() {
    let lib_dir = home_dir().expect("bruh you have no home directory").join("librarygrid");
    if let Err(_) = std::fs::read_dir(&lib_dir) {
        first_time(lib_dir.clone());
    }
    match check_ver(lib_dir.clone()) {
        Ok(()) => {},
        Err(e) => {
            eprintln!("there was an error: {}", e);
            std::process::exit(1);
        }
    }
}

fn first_time(lib_dir: PathBuf) {
    println!("welcome to the librarygrid software!");
    println!("it appears you're a new user. would you like to install the software? (y/n) (installs to `{}`)", lib_dir.display());
    let mut yn = String::new();
    let _ = stdout().flush();
    stdin().read_line(&mut yn).expect("please enter a valid string!");
    let fc = yn.chars().next().unwrap();
    if fc != 'y' && fc != 'Y' {
        println!("goodbye!");
        std::process::exit(0);
    }

    match std::fs::create_dir(&lib_dir) {
        Ok(_) => println!("directory created successfully!"),
        Err(e) => {
            eprintln!("there was an error: {}", e);
            std::process::exit(1);
        }
    }
    
    match std::fs::create_dir(&lib_dir.join("web")) {
        Ok(_) => println!("web directory created successfully!"),
        Err(e) => {
            eprintln!("there was an error: {}", e);
            std::process::exit(1);
        }
    }

    match extract_files(lib_dir) {
        Ok(()) => println!("web dir was extracted successfully!"),
        Err(e) => {
            eprintln!("there was an error: {}", e);
            std::process::exit(1);
        }
    }
}

fn check_ver(lib_dir: PathBuf) -> Result<(), std::io::Error> {
    let mut version_file = File::open(&lib_dir.join("web/version.txt"))?;
    let mut cur_version = String::new();
    version_file.read_to_string(&mut cur_version)?;
    println!("current version: {}", cur_version);
    if WEB_VERSION != cur_version {
        println!("down-/upgrading to web interface version: {}", WEB_VERSION);
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
