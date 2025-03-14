use flate2::read::GzDecoder;
use nix::libc::dev_t;
use nix::sys::stat::{mknod, Mode};
use nix::unistd::mkfifo;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::{fs, io};

#[derive(Debug)]
pub struct UStarHeader {
    name: [u8; 100],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 12],
    modification_time: [u8; 12],
    checksum: [u8; 8],
    type_flag: [u8; 1],
    link_name: [u8; 100],
    ustar: [u8; 6],
    version: [u8; 2],
    user_name: [u8; 32],
    group_name: [u8; 32],
    device_major: [u8; 8],
    device_minor: [u8; 8],
    prefix: [u8; 155],
    padding: [u8; 12],
}
impl UStarHeader {
    fn new() -> Self {
        UStarHeader {
            name: [0; 100],
            mode: [0; 8],
            uid: [0; 8],
            gid: [0; 8],
            size: [0; 12],
            modification_time: [0; 12],
            checksum: [0; 8],
            type_flag: [0; 1],
            link_name: [0; 100],
            ustar: [0; 6],
            version: [0; 2],
            user_name: [0; 32],
            group_name: [0; 32],
            device_major: [0; 8],
            device_minor: [0; 8],
            prefix: [0; 155],
            padding: [0; 12],
        }
    }
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut header = UStarHeader::new();

        header.name.copy_from_slice(&bytes[0..100]);
        header.mode.copy_from_slice(&bytes[100..108]);
        header.uid.copy_from_slice(&bytes[108..116]);
        header.gid.copy_from_slice(&bytes[116..124]);
        header.size.copy_from_slice(&bytes[124..136]);
        header.modification_time.copy_from_slice(&bytes[136..148]);
        header.checksum.copy_from_slice(&bytes[148..156]);
        header.type_flag.copy_from_slice(&bytes[156..157]);
        header.link_name.copy_from_slice(&bytes[157..257]);
        header.ustar.copy_from_slice(&bytes[257..263]);
        header.version.copy_from_slice(&bytes[263..265]);
        header.user_name.copy_from_slice(&bytes[265..297]);
        header.group_name.copy_from_slice(&bytes[297..329]);
        header.device_major.copy_from_slice(&bytes[329..337]);
        header.device_minor.copy_from_slice(&bytes[337..345]);
        header.prefix.copy_from_slice(&bytes[345..500]);
        header.padding.copy_from_slice(&bytes[500..512]);

        header
    }
    fn file_name(&self) -> String {
        let mut name = String::from_utf8(self.name.to_vec())
            .unwrap()
            .trim_end_matches('\0')
            .to_string();
        let prefix = String::from_utf8(self.prefix.to_vec())
            .unwrap()
            .trim_end_matches('\0')
            .to_string();
        if !prefix.is_empty() {
            name = format!("{}/{}", prefix, name);
        }
        name
    }
    fn file_size(&self) -> usize {
        usize::from_str_radix(
            std::str::from_utf8(&self.size)
                .unwrap_or("0")
                .trim_end_matches('\0'),
            8,
        )
        .unwrap_or(0)
    }
    fn get_mode(&self) -> Mode {
        let mode_str = String::from_utf8_lossy(&self.mode)
            .trim_end_matches('\0')
            .to_string();

        u32::from_str_radix(&mode_str, 8)
            .map(Mode::from_bits_truncate)
            .unwrap()
    }
}
pub fn extract_files(tar_file: &str, is_compressed: bool) -> Result<(), io::Error> {
    let file = File::open(tar_file)?;

    let mut reader: Box<dyn Read> = if is_compressed {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut buffer = vec![0; 512];

    while reader.read_exact(&mut buffer).is_ok() {
        if buffer.iter().all(|&b| b == 0) {
            break;
        }
        let header = UStarHeader::from_bytes(&buffer);
        let file_name = header.file_name();
        let file_size = header.file_size();
        let type_flag = header.type_flag[0] as char;
        let mode = header.get_mode();
        let major = usize::from_str_radix(
            std::str::from_utf8(&header.device_major)
                .unwrap()
                .trim_end_matches('\0')
                .trim(),
            8,
        )
        .unwrap_or(0);
        let minor = usize::from_str_radix(
            std::str::from_utf8(&header.device_minor)
                .unwrap()
                .trim_end_matches('\0')
                .trim(),
            8,
        )
        .unwrap_or(0);
        match type_flag {
            '0' => {
                let mut content = vec![0; file_size];
                reader.read_exact(&mut content)?;
                if let Some(parent) = Path::new(&file_name).parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut output_file = File::create(&file_name)?;
                output_file.write_all(&content)?;

                let padding = (512 - (file_size % 512)) % 512;
                reader.read_exact(&mut vec![0; padding])?;
            }
            '1' => {
                let link_target = String::from_utf8(Vec::from(&header.link_name))
                    .unwrap()
                    .trim_end_matches('\0')
                    .to_string();
                fs::hard_link(link_target, &file_name)?;
            }
            '2' => {
                let link_target = String::from_utf8(Vec::from(&header.link_name))
                    .unwrap()
                    .trim_end_matches('\0')
                    .to_string();
                std::os::unix::fs::symlink(&link_target, &file_name)?;
            }
            '3' => {
                match mknod(
                    Path::new(&file_name),
                    nix::sys::stat::SFlag::S_IFCHR,
                    mode,
                    ((major << 8) | minor) as dev_t,
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("Error:{e}.Run with sudo!\n\
                                    Use cargo build --release \n\
                                    Than execute sudo ./target/release/project ");
                    }
                }
            }
            '4' => {
                match mknod(
                    Path::new(&file_name),
                    nix::sys::stat::SFlag::S_IFBLK,
                    mode,
                    ((major << 8) | minor) as dev_t,
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("Error:{e}.Run with sudo!\n\
                                    Use cargo build --release \n\
                                    Than execute sudo ./target/release/project ");
                    }
                }
            }
            '5' => {
                if Path::new(&file_name).exists() {
                    println!("Directory '{}' already exists.", file_name);
                    println!("Do you want to overwrite it? (y/n): ");

                    let mut response = String::new();
                    io::stdin().read_line(&mut response)?;

                    if response.trim().to_lowercase() == "y" {
                        println!("Overwriting directory: {}", file_name);
                        fs::remove_dir_all(&file_name)?;
                        fs::create_dir(&file_name)?;
                    } else {
                        fs::create_dir(&file_name)?;
                    }
                }
            }
            '6' => {
                mkfifo(Path::new(&file_name), mode)?;
            }
            _ => {
                println!("Unknown type flag: {}", type_flag);
            }
        }
    }
    Ok(())
}
