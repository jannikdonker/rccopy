use std::{path::PathBuf, time::{SystemTime, Instant, Duration}, fs, io::{Read, Write, BufWriter}};
use clap::Parser;
use chrono::{DateTime, Utc, SecondsFormat};
use xxhash_rust::xxh64::Xxh64;
use md5::{Md5, Digest};
use sha1::Sha1;
use filetime_creation::FileTime;
use quick_xml::Writer;
use whoami;

#[derive(Parser, Debug)]
#[clap(name = "rccopy", about = "Copies a given input directory to a new destination directory while preserving the directory structure using checksums to verify that the files are identical after copying. Can write a mhl (MediaHashList) file containing the checksums of the copied files to the destination directory.")]
struct Opt {
    /// Input directory
    #[clap(short, long, required(true), help = "The source directory to copy.")]
    input: PathBuf,

    /// Destination directory
    #[clap(short, long, required(true), help = "The target directory to copy to.")]
    destination: PathBuf,

    /// Checksum method. Possible checksums: md5, sha1, xxhash64
    #[clap(short, long, help = "The checksum method to use. Possible checksums: md5, sha1, xxhash64.")]
    checksum: Option<String>,

    /// Write a mhl file to the destination directory
    #[clap(short, long, help = "Write a mhl file to the destination directory.")]
    mhl: bool,

    /// Dry run. Preview the files that will be copied.
    #[clap(long, help = "Preview the files that will be copied.")]
    dry_run: bool,
}

// Struct to hold the metadata of a file for the MediaHashList.
struct FileMetadata {
    file: String,
    size: u64,
    last_modification_date: SystemTime,
    checksum: String,
    checksum_method: String,
    hash_date: SystemTime,
}

enum HashMethod {
    Md5(Md5),
    Sha1(Sha1),
    Xxh64(Xxh64),
}

// The size of the chunks to read from the input file. 8MB.
const CHUNK_SIZE: usize = 1024 * 1024 * 8;

fn main () {
    let start_date = format_system_time_to_rfc3339(SystemTime::now());
    let start_date_for_file_name: String = start_date.replace(":", "").replace("T", "_").replace("Z", "");
    println!("Start date: {}", start_date);

    let opt: Opt = Opt::parse();

    // Check if the input and destination directorys exist. Print as Error.
    if !opt.input.exists() {
        eprintln!("Error: Input directory does not exist.");
        std::process::exit(1);
    }
    if !opt.destination.exists() {
        eprintln!("Error: Destination directory does not exist.");
        std::process::exit(1);
    }

    // Check if the input and destination directorys are directories. Print as Error.
    if !opt.input.is_dir() {
        eprintln!("Error: Input is not a directory.");
        std::process::exit(1);
    }
    if !opt.destination.is_dir() {
        eprintln!("Error: Destination is not a directory.");
        std::process::exit(1);
    }

    // Check if the input and destination directorys are the same. Print as Error.
    if opt.input == opt.destination {
        eprintln!("Error: Input and destination directorys are the same.");
        std::process::exit(1);
    }

    // Search the input directory recursively for files.
    let files: Vec<PathBuf> = get_files_in_directory(&opt.input);

    // Initialze some stuff
    let mut failed_files: Vec<PathBuf> = Vec::new();
    let mut had_errors = false;
    let mut copied_anything = false;
    let total_files = files.len();
    let mut mhl_data: Vec<FileMetadata> = Vec::new();

    // Copy the files.
    for file in &files {
        let destination_file: PathBuf = opt.destination.join(file.strip_prefix(&opt.input).unwrap());

        // Check if the file already exists in the destination directory. Verify that the file sizes match.
        if destination_file.exists() && destination_file.metadata().unwrap().len() == file.metadata().unwrap().len() {
            println!("-------------------------");
            println!("{} / {}: File {} already exists and has identical file size. Skipping...", files.iter().position(|x| x == file).unwrap() + 1, total_files, destination_file.display());
            continue;
        }

        println!("-------------------------");
        println!("{} / {}: {} --> {}", files.iter().position(|x| x == file).unwrap() + 1, total_files, file.display(), destination_file.display());

        if opt.dry_run {
            continue;
        }

        let src_checksum = copy_file(file, &destination_file, &opt.checksum);

        if src_checksum.is_err() {
            eprintln!("Error: Could not copy file.");
            failed_files.push(file.clone());
            had_errors = true;
            continue;  
        } else if src_checksum.as_ref().unwrap() == "None" {
            copied_anything = true;
            println!();
            continue;
        } else {
            copied_anything = true;

            println!("Verifying checksum... ({})", opt.checksum.as_ref().unwrap());

            let dest_checksum = process_checksum(&destination_file.to_str().unwrap(), &opt.checksum);

            if dest_checksum.is_err() {
                eprintln!("Error: Could not verify checksum.");
                failed_files.push(file.clone());
                had_errors = true;
                continue;
            } else if src_checksum.as_ref().unwrap() == dest_checksum.as_ref().unwrap() {
                println!("Checksums match: {}", src_checksum.as_ref().unwrap());
                mhl_data.push(FileMetadata {
                    file: destination_file.strip_prefix(&opt.destination).unwrap().to_str().unwrap().to_string(),
                    size: file.metadata().unwrap().len(),
                    last_modification_date: file.metadata().unwrap().modified().unwrap(),
                    checksum: src_checksum.unwrap(),
                    checksum_method: opt.checksum.as_ref().unwrap().to_string(),
                    hash_date: SystemTime::now(),
                });
                continue;
            } else {
                println!("Error: Checksums do not match. File was not copied successfully.");
                failed_files.push(file.clone());
                had_errors = true;
                continue;
            }
        }
    }

    if opt.mhl && copied_anything && !opt.dry_run {
        println!("-------------------------");
        println!("Writing mhl file...");

        // MHL file name is the basedir of the source directory + the current date and time + .mhl
        let mhl_file = opt.destination.join(format!("{}-{}.mhl", opt.input.file_name().unwrap().to_str().unwrap(), start_date_for_file_name));

        let mhl_result = write_mhl(&mhl_file, mhl_data, start_date);

        if mhl_result.is_err() {
            eprintln!("Error: Could not write mhl file.");
            std::process::exit(1);
        }
    }

    println!("-------------------------");

    if opt.dry_run {
        println!("Finished dry run.");
    } else if had_errors {
        println!("Finished with errors.");
        println!("Failed files:");
        for file in failed_files {
            println!("{}", file.display());
        }
    } else if copied_anything{
        println!("Finished successfully. 🎉");
    } else {
        println!("Nothing to copy.");
    }
}

// Searches the given directory recursively for files and returns a vector of the files.
fn get_files_in_directory(dir: &PathBuf) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            files.append(&mut get_files_in_directory(&path));
        } else {
            files.push(path);
        }
    }

    files
}

// Copy a file from the input directory to the destination directory.
fn copy_file (input_path: &PathBuf, destination_path: &PathBuf, checksum_method: &Option<String>) -> Result<String, std::io::Error> {

    // Create the destination directory if it doesnt exist.
    if !destination_path.parent().unwrap().exists() {
        fs::create_dir_all(destination_path.parent().unwrap()).unwrap();
    }

    // Open the input file.
    let mut input_file = fs::File::open(input_path).unwrap();

    // Create the destination file.
    let mut destination_file = fs::File::create(destination_path).unwrap();

    // Initialize some variables.
    let mut buffer = vec![0; CHUNK_SIZE];
    let mut total_bytes_read = 0;
    let mut last_print_time = Instant::now();

    // Check if a checksum method was given.
    if checksum_method.is_some() {

        let mut hasher: HashMethod = match checksum_method.as_ref().unwrap().as_str() {
            "md5" => HashMethod::Md5(Md5::new()),
            "sha1" => HashMethod::Sha1(Sha1::new()),
            "xxhash64" => HashMethod::Xxh64(Xxh64::new(0)),
            _ => {
                eprintln!("Error: Invalid checksum method.");
                std::process::exit(1);
            }
        };

        // Print a placeholder for the transfer speed.
        print!("\rTransfer speed: {:30}\r", "---.-- MB/s");
        std::io::stdout().flush().unwrap();

        // Copy the file. With checksum.
        loop {
            let bytes_read = input_file.read(&mut buffer).unwrap();

            if bytes_read == 0 {
                break;
            }
            destination_file.write_all(&buffer[..bytes_read]).unwrap();

            // Update hash
            match &mut hasher {
                HashMethod::Md5(h) => h.update(&buffer[..bytes_read]),
                HashMethod::Sha1(h) => h.update(&buffer[..bytes_read]),
                HashMethod::Xxh64(h) => h.update(&buffer[..bytes_read]),
            };

            total_bytes_read += bytes_read;
        
            // Print transfer speed every 100 ms. Use the format bytes function to format the bytes.
            let elapsed = last_print_time.elapsed();

            if elapsed > Duration::from_millis(100) {
                std::io::stdout().flush().unwrap();
                let bytes_per_second = total_bytes_read as f64 / elapsed.as_secs_f64();
                print!("\rTransfer speed: {:30}\r", format_bytes(bytes_per_second as u64));
                last_print_time = Instant::now();
                total_bytes_read = 0;  // reset total_bytes_read here
            }
        }

        // Compute and return the checksum
        let hash_string = match hasher {
            HashMethod::Md5(h) => format!("{:x}", h.finalize()),
            HashMethod::Sha1(h) => format!("{:x}", h.finalize()),
            HashMethod::Xxh64(h) => format!("{:x}", h.digest()),
        };

        // Copy the metadata
        let metadata = std::fs::metadata(input_path)?;
        let permissions = metadata.permissions();
        std::fs::set_permissions(destination_path, permissions)?;

        let accessed = FileTime::from_last_access_time(&metadata);
        let modified = FileTime::from_last_modification_time(&metadata);
        let created = FileTime::from_creation_time(&metadata);

        filetime_creation::set_file_times(destination_path, accessed, modified, created.unwrap())?;

        Ok(hash_string)

    } else {
        // Print a placeholder for the transfer speed.
        print!("\rTransfer speed: {:30}\r", "---.-- MB/s");
        std::io::stdout().flush().unwrap();

        // Copy the file.
        loop {
            let bytes_read = input_file.read(&mut buffer).unwrap();
            if bytes_read == 0 {
                break;
            }
            destination_file.write_all(&buffer[..bytes_read]).unwrap();
            total_bytes_read += bytes_read;
        
            // Print transfer speed every 100 ms. Use the format bytes function to format the bytes.
            let elapsed = last_print_time.elapsed();

            if elapsed > Duration::from_millis(100) {
                std::io::stdout().flush().unwrap();
                let bytes_per_second = total_bytes_read as f64 / elapsed.as_secs_f64();
                print!("\rTransfer speed: {:30}\r", format_bytes(bytes_per_second as u64));
                last_print_time = Instant::now();
                total_bytes_read = 0;  // reset total_bytes_read here
            }
        }

        // Copy the metadata
        let metadata = std::fs::metadata(input_path)?;
        let permissions = metadata.permissions();
        std::fs::set_permissions(destination_path, permissions)?;

        let accessed = FileTime::from_last_access_time(&metadata);
        let modified = FileTime::from_last_modification_time(&metadata);
        let created = FileTime::from_creation_time(&metadata);

        filetime_creation::set_file_times(destination_path, accessed, modified, created.unwrap())?;

        Ok("None".to_string())
    }
}

// Process the checksum of a file.
fn process_checksum(input_file: &str, checksum_method: &Option<String>) -> Result<String, std::io::Error> {

    let mut buffer = vec![0; CHUNK_SIZE];

    // Open the input file.
    let mut input_file = fs::File::open(input_file).unwrap();

    let mut hasher: HashMethod = match checksum_method.as_ref().unwrap().as_str() {
        "md5" => HashMethod::Md5(Md5::new()),
        "sha1" => HashMethod::Sha1(Sha1::new()),
        "xxhash64" => HashMethod::Xxh64(Xxh64::new(0)),
        _ => {
            eprintln!("Error: Invalid checksum method.");
            std::process::exit(1);
        }
    };

    // Calculate the checksum of the file.
    loop {
        let bytes_read = input_file.read(&mut buffer).unwrap();

        if bytes_read == 0 {
            break;
        }

        // Update hash
        match &mut hasher {
            HashMethod::Md5(h) => h.update(&buffer[..bytes_read]),
            HashMethod::Sha1(h) => h.update(&buffer[..bytes_read]),
            HashMethod::Xxh64(h) => h.update(&buffer[..bytes_read]),
        };
    }

    // Compute and return the checksum
    let hash_string = match hasher {
        HashMethod::Md5(h) => format!("{:x}", h.finalize()),
        HashMethod::Sha1(h) => format!("{:x}", h.finalize()),
        HashMethod::Xxh64(h) => format!("{:x}", h.digest()),
    };

    Ok(hash_string)

}

// Formats a SystemTime to a RFC3339 string.
fn format_system_time_to_rfc3339(st: SystemTime) -> String {
    let datetime: DateTime<Utc> = st.into();
    datetime.to_rfc3339_opts(SecondsFormat::Secs, true)
}

// Formats Bytes to a human readable string.
fn format_bytes(bytes: u64) -> String {
    let kb: u64 = 1024;
    let mb: u64 = kb * 1024;
    let gb: u64 = mb * 1024;
    let tb: u64 = gb * 1024;

    if bytes < kb {
        format!("{} B/s", bytes)
    } else if bytes < mb {
        format!("{:.2} KB/s", bytes as f64 / kb as f64)
    } else if bytes < gb {
        format!("{:.2} MB/s", bytes as f64 / mb as f64)
    } else if bytes < tb {
        format!("{:.2} GB/s", bytes as f64 / gb as f64)
    } else {
        format!("{:.2} TB/s", bytes as f64 / tb as f64)
    }
}

// Write a mhl file to the destination directory.
fn write_mhl(destination_path: &PathBuf, metadata: Vec<FileMetadata>, start_date: String) -> quick_xml::Result<()> {
    let file = fs::File::create(&destination_path)?;
    let mut writer = Writer::new(BufWriter::new(file));

    writer.write(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")?;
    writer.write(b"<hashlist version=\"1.1\">\n\n")?;

    // Reading the system information
    let computer_name = whoami::devicename();
    let hostname = whoami::hostname();
    let username = whoami::username();
    let tool = format!("{} ver. {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    let finish_date = format_system_time_to_rfc3339(SystemTime::now());

    writer.write(b"  <creatorinfo>\n")?;
    writer.write(b"    <name>")?;
    writer.write(computer_name.as_bytes())?;
    writer.write(b"</name>\n")?;
    writer.write(b"    <username>")?;
    writer.write(username.as_bytes())?;
    writer.write(b"</username>\n")?;
    writer.write(b"    <hostname>")?;
    writer.write(hostname.as_bytes())?;
    writer.write(b"</hostname>\n")?;
    writer.write(b"    <tool>")?;
    writer.write(tool.as_bytes())?;
    writer.write(b"</tool>\n")?;
    writer.write(b"    <startdate>")?;
    writer.write(start_date.as_bytes())?;
    writer.write(b"</startdate>\n")?;
    writer.write(b"    <finishdate>")?;
    writer.write(finish_date.as_bytes())?;
    writer.write(b"</finishdate>\n")?;
    writer.write(b"  </creatorinfo>\n\n")?;

    for item in metadata {
        writer.write(b"  <hash>\n")?;

        let file_path = PathBuf::from(&item.file);
        let relative_path = file_path.strip_prefix(&destination_path).unwrap_or(&file_path);

        writer.write(b"    <file>")?;
        writer.write(relative_path.to_string_lossy().as_bytes())?;
        writer.write(b"</file>\n")?;

        writer.write(b"    <size>")?;
        writer.write(item.size.to_string().as_bytes())?;
        writer.write(b"</size>\n")?;

        writer.write(b"    <lastmodificationdate>")?;
        let _ = writer.write(format_system_time_to_rfc3339(item.last_modification_date).as_bytes());
        writer.write(b"</lastmodificationdate>\n")?;

        writer.write(format!("    <{}>", item.checksum_method).as_bytes())?;
        writer.write(item.checksum.as_bytes())?;
        writer.write(format!("</{}>\n", item.checksum_method).as_bytes())?;

        writer.write(b"    <hashdate>")?;
        let _ = writer.write(format_system_time_to_rfc3339(item.hash_date).as_bytes());
        writer.write(b"</hashdate>\n")?;

        writer.write(b"  </hash>\n\n")?;
    }

    writer.write(b"</hashlist>\n")?;

    Ok(())
}
