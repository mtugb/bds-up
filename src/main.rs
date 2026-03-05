use std::error::Error;
use std::fs;
use std::io;
use std::io::Cursor;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use clap::Parser;
use console::style;
use dialoguer::Confirm;
use futures::StreamExt;
use indexmap::IndexMap;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use serde::Deserialize;
use serde::Serialize;
use tempfile::TempDir;
use zip::ZipArchive;

/// Bedrock Dedicated Server Specialized Updating tool.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    stable: bool,

    #[arg(short, long)]
    preview: bool,

    #[arg(short, long)]
    no_backup: bool,

    #[arg(short, long, default_value = ".")]
    target_dir: PathBuf,
}

fn parse_properties(raw: &str) -> IndexMap<&str, &str> {
    let mut propmap = IndexMap::new();
    for line in raw.lines() {
        if line.starts_with("#") || line.is_empty() {
            continue;
        };
        let mut splited = line.split("=");
        let key = splited.next().unwrap();
        let value = splited.next().expect("Illegal Syntax");
        propmap.insert(key, value);
    }
    propmap
}

// -----
// Source - https://stackoverflow.com/a/65192210
// Posted by Simon Buchan, modified by community. See post 'Timeline' for change history
// Retrieved 2026-02-28, License - CC BY-SA 4.0

use std::path::Path;

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
// -----

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct Manifest {
    cdn_root: String,
    linux: LinuxUpdates,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct LinuxUpdates {
    stable: String,
    preview: String,
    versions: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
struct VersionDetail {
    download_url: String,
}

async fn get_dl_url_with_interection(args: &Args) -> Result<String, Box<dyn Error>> {
    const API_BASE: &str = "https://raw.githubusercontent.com/Bedrock-OSS/BDS-Versions/main";
    let manifest_json = reqwest::get(format!("{}/versions.json", API_BASE))
        .await?
        .text()
        .await?;
    let manifest: Manifest = serde_json::from_str(&manifest_json)?;

    let options = vec![
        format!(
            "{} {}",
            style("Stable (recommended)").green().bold(),
            manifest.linux.stable
        ),
        format!(
            "{} {}",
            style("Preview").blue().bold(),
            manifest.linux.preview
        ),
    ];

    let raw_versions = [&manifest.linux.stable, &manifest.linux.preview];

    let selected = if args.stable {
        0
    } else if args.preview {
        1
    } else {
        dialoguer::FuzzySelect::new()
            .with_prompt("Which version do you prefer?")
            .default(0)
            .items(options)
            .interact()?
    };

    let parent_dir = ["linux", "linux_preview"];

    let version_detail_json = reqwest::get(format!(
        "{}/{}/{}.json",
        API_BASE, parent_dir[selected], raw_versions[selected]
    ))
    .await?
    .text()
    .await?;
    let version_detail: VersionDetail = serde_json::from_str(&version_detail_json)?;
    Ok(version_detail.download_url)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    run_update(args).await?;
    Ok(())
}

async fn run_update(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let target_dir = &args.target_dir;
    println!(
        "Going to update BDS server in {}",
        dunce::canonicalize(target_dir)?.to_str().unwrap()
    );
    let starting_confirm = Confirm::new()
        .with_prompt("Do you want to continue?")
        .interact()?;
    if !starting_confirm {
        println!("{}", style("Update cancelled.").yellow());
        return Ok(());
    }
    let backup_needed = !args.no_backup
        && Confirm::new()
            .with_prompt("Do you need backup?")
            .interact()?;

    // Loads current settings
    let current_config_path = target_dir.join("server.properties");
    let current_config_raw = fs::read_to_string(current_config_path)?;
    let current_config = parse_properties(&current_config_raw);
    // println!("{:?}", current_config);

    // Prepare resource target url
    let zip_url = get_dl_url_with_interection(&args).await?;
    // Loads latest settings
    let zip_fetch_pb = ProgressBar::new_spinner();
    zip_fetch_pb.enable_steady_tick(Duration::from_millis(120));
    zip_fetch_pb.set_message("Connecting to mojang server...");
    let zip_response = reqwest::get(zip_url).await?;
    let zip_total_size = zip_response
        .content_length()
        .ok_or("長さを取得できませんでした。")?;
    zip_fetch_pb.finish_with_message(format!(
        "{} Connection established",
        style("✓").green().bold()
    ));
    let zip_write_pb = ProgressBar::new(zip_total_size);
    zip_write_pb.set_style(ProgressStyle::default_bar().progress_chars("#>-"));
    let mut zip_bytes_stream = zip_response.bytes_stream();
    let mut zip_blob = Cursor::new(Vec::new());
    let mut downloaded: u64 = 0;
    println!("● Downloading resources...");
    while let Some(item) = zip_bytes_stream.next().await {
        let chunk = item?;
        zip_blob.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        zip_write_pb.set_position(downloaded);
    }
    zip_write_pb.finish_with_message(format!(
        "{} Deployment successful!",
        style("✓").green().bold()
    ));
    let mut zip_file = ZipArchive::new(zip_blob)?;
    let latest_config_blob = zip_file.by_name("server.properties")?;
    let latest_config_raw = io::read_to_string(latest_config_blob)?;
    let latest_config = parse_properties(&latest_config_raw);

    // Merges settings
    let mut merged_config = current_config.clone();
    for key in latest_config.keys() {
        // If exsists, skipped
        merged_config
            .entry(key)
            .or_insert(latest_config.get(key).unwrap());
    }

    println!("● Copying downloaded resources...");
    let copy_pb = ProgressBar::new(zip_file.len() as u64);
    copy_pb.set_style(ProgressStyle::default_bar().progress_chars("#>-"));

    // Extracts latests
    let latest_root = TempDir::new()?;
    for i in 0..zip_file.len() {
        let mut entry = zip_file.by_index(i)?;
        let relative_path = entry.name();
        let entry_path = latest_root.path().join(relative_path);
        if entry.is_dir() {
            fs::create_dir_all(entry_path)?;
        } else {
            if let Some(parent_dir) = entry_path.parent() {
                fs::create_dir_all(parent_dir)?;
            }
            let mut target = fs::File::create(&entry_path)?;
            io::copy(&mut entry, &mut target)?;
        }
        copy_pb.set_position(i as u64);
    }
    copy_pb.finish_with_message(format!("{} Copying successful!", style("✓").green().bold()));

    // Puts Merged settings: server.properties
    let mut merged_config_content = String::new();
    for entry in merged_config.iter() {
        let (key, value) = entry;
        merged_config_content.push_str(&format!("{}={}\n", key, value));
    }
    fs::write(
        latest_root.path().join("server.properties"),
        merged_config_content,
    )?;

    let file_to_copy = [
        "allowlist.json",
        "permissions.json",
        "valid_known_packs.json", // TODO: implement process to update this in case of vanilla-
        // updates
        "world_behavior_packs.json",
        "world_resource_packs.json",
    ];
    for file in file_to_copy {
        let from = target_dir.join(file);
        let to = latest_root.path().join(file);
        if Path::new(&from).exists() {
            fs::copy(from, to)?;
        }
    }
    let dir_to_copy = [
        "worlds/",
        "development_behavior_packs/",
        "development_resource_packs/",
        "resource_packs/",
        "behavior_packs/",
    ];
    for dir in dir_to_copy {
        let from = target_dir.join(dir);
        let to = latest_root.path().join(dir);
        if Path::new(&from).exists() {
            copy_dir_all(from, to)?;
        }
    }

    if backup_needed {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%SZ").to_string();
        let backup_path = PathBuf::from(format!("{}-backup-{}", target_dir.display(), timestamp));
        fs::rename(target_dir, backup_path)?;
    } else {
        fs::remove_dir_all(target_dir)?;
    }

    copy_dir_all(latest_root.path(), target_dir)?;

    #[cfg(unix)] // Linux/macOSの場合のみ実行
    {
        use std::os::unix::fs::PermissionsExt;
        let binary_path = target_dir.join("bedrock_server");
        if binary_path.exists() {
            let mut perms = fs::metadata(&binary_path)?.permissions();
            perms.set_mode(0o755); // 実行可能権限を付与
            fs::set_permissions(&binary_path, perms)?;
        }
    }

    println!(
        "{} completed successfully!☆彡",
        style("Updating").green().bold()
    );

    Ok(())
}
