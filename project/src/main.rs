use crate::pack_unpack::packing::create_archive;
use crate::pack_unpack::unpacking::extract_files;
use std::io;
mod pack_unpack;
fn execute_command(command: Vec<&str>) {
    if command.len() < 2 {
        println!("Invalid command. Use <.tar --help> to find out more.");
        return;
    }

    match command[1] {
        "--help" => {
            println!(
                "This tool packs or unpacks files in the format .tar or .tar.gz.\n\
                To pack a directory, use the following format:\n\
                1. For .tar: .tar pack <path_to_directory> [<name_of_archive>]\n\
                2. For .tar.gz: .tar pack <path_to_directory> -c [<name_of_archive>]\n\
                If you don't specify the name, a generic archive.tar or archive.tar.gz will be created.\n\
                Don't include extensions in the name.\n\
                To unpack, use the following format:\n\
                .tar unpack <path_to_archive>\n\
                To close the tool use quit."
            );
        }
        "pack" => {
            if command.len() < 3 {
                println!("Invalid command. Specify the path to the directory to pack. Use <.tar --help> to find out more.");
                return;
            }

            let path_to_directory = command[2];
            let compress = command.get(3) == Some(&"-c");
            let archive_name = if compress {
                command.get(4).unwrap_or(&"archive").to_string()
            } else {
                command.get(3).unwrap_or(&"archive").to_string()
            };

            match create_archive(path_to_directory, &archive_name, compress) {
                Ok(_) => {
                    let extension = if compress { ".tar.gz" } else { ".tar" };
                    println!("Successfully created {}{}", archive_name, extension);
                }
                Err(e) => println!("Error packing archive: {}", e),
            }
        }
        "unpack" => {
            if command.len() < 3 {
                println!(
                    "Invalid command. Specify the archive. Use <.tar --help> to find out more."
                );
                return;
            }

            let archive_path = command[2];
            let is_compressed = archive_path.as_bytes()[archive_path.len() - 1] == b'z'
                && archive_path.as_bytes()[archive_path.len() - 2] == b'g';
            if !archive_path.ends_with(".tar") || !archive_path.ends_with(".tar.gz") {
                println!("Unsupported file type!");
            } else {
                match extract_files(archive_path, is_compressed) {
                    Ok(_) => println!("Successfully unpacked {}", archive_path),
                    Err(e) => println!("Error unpacking archive: {}", e),
                }
            }
        }
        _ => {
            println!("Unknown command. Use <.tar --help> to find out more.");
        }
    }
}
fn main() -> io::Result<()> {
    println!("Hello to my .tar tool.Use <.tar --help> to find out more.");
    loop {
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");
        let args: Vec<&str> = input.split_whitespace().collect();

        if args[0] == "quit" {
            println!("Exiting...");
            return Ok(());
        } else if args.len() < 2 || args[0] != ".tar" {
            println!("Invalid command.Use <.tar --help> to find out more.");
        } else {
            execute_command(args);
        }
    }
}
