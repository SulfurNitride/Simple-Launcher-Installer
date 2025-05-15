use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;
use std::thread;
use std::time::Duration;
use reqwest::blocking::Client;
use regex::Regex;
use std::os::unix::fs::PermissionsExt;

// ANSI color codes
const COLOR_GREEN: &str = "\x1b[0;32m";
const COLOR_YELLOW: &str = "\x1b[0;33m";
const COLOR_RED: &str = "\x1b[0;31m";
const COLOR_BLUE: &str = "\x1b[0;34m";
const COLOR_RESET: &str = "\x1b[0m";

// Struct to hold application paths
struct AppPaths {
    home_dir: PathBuf,
    battlenet_installer: PathBuf,
    hoyoplay_installer: PathBuf,
}

// Find system wine installation
fn find_system_wine() -> Option<String> {
    println!("{}Searching for system wine installation...{}", COLOR_BLUE, COLOR_RESET);

    // Check common wine paths
    let wine_paths = vec![
        PathBuf::from("/usr/bin/wine"),
        PathBuf::from("/usr/local/bin/wine"),
    ];

    // First try the PATH environment variable
    if let Ok(output) = Command::new("which").arg("wine").output() {
        if output.status.success() {
            let path = str::from_utf8(&output.stdout).unwrap_or("").trim();
            if !path.is_empty() {
                let path_buf = PathBuf::from(path);
                if path_buf.exists() {
                    // Get wine version
                    if let Ok(version_output) = Command::new(&path_buf).arg("--version").output() {
                        if version_output.status.success() {
                            let version = str::from_utf8(&version_output.stdout).unwrap_or("unknown").trim();
                            println!("{}Found system wine at: {}{}", COLOR_GREEN, path, COLOR_RESET);
                            println!("{}Wine version: {}{}", COLOR_GREEN, version, COLOR_RESET);
                            return Some(path.to_string());
                        }
                    }
                }
            }
        }
    }

    // Then check common paths
    for path in wine_paths {
        if path.exists() {
            if let Ok(metadata) = fs::metadata(&path) {
                if metadata.is_file() {
                    // Try to get wine version
                    if let Ok(output) = Command::new(&path).arg("--version").output() {
                        if output.status.success() {
                            let version = str::from_utf8(&output.stdout).unwrap_or("unknown").trim();
                            println!("{}Found system wine at: {}{}", COLOR_GREEN, path.display(), COLOR_RESET);
                            println!("{}Wine version: {}{}", COLOR_GREEN, version, COLOR_RESET);
                            return Some(path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }

    println!("{}Error: System wine installation not found.{}", COLOR_RED, COLOR_RESET);
    println!("Please install wine using your distribution's package manager.");
    println!("Example: sudo apt install wine    # For Debian/Ubuntu");
    println!("         sudo dnf install wine    # For Fedora");
    println!("         sudo pacman -S wine      # For Arch Linux");

    None
}



// Download a file
fn download_file(url: &str, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        println!("{}File already exists at {}. Skipping download.{}",
                 COLOR_YELLOW, destination.display(), COLOR_RESET);
        return Ok(());
    }

    // Create parent directories if they don't exist
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    println!("{}Downloading file from {}...{}", COLOR_BLUE, url, COLOR_RESET);

    // Try to use wget or curl if available
    if Command::new("which").arg("curl").status().is_ok() {
        let status = Command::new("curl")
        .arg("-L")
        .arg("-o")
        .arg(destination)
        .arg(url)
        .status()
        .map_err(|e| format!("Failed to execute curl: {}", e))?;

        if status.success() {
            println!("{}Download complete!{}", COLOR_GREEN, COLOR_RESET);
            return Ok(());
        } else {
            return Err(format!("curl failed with exit code: {}", status));
        }
    } else if Command::new("which").arg("wget").status().is_ok() {
        let status = Command::new("wget")
        .arg("-O")
        .arg(destination)
        .arg(url)
        .status()
        .map_err(|e| format!("Failed to execute wget: {}", e))?;

        if status.success() {
            println!("{}Download complete!{}", COLOR_GREEN, COLOR_RESET);
            return Ok(());
        } else {
            return Err(format!("wget failed with exit code: {}", status));
        }
    } else {
        // Fallback to using reqwest
        let client = Client::new();
        let response = client.get(url)
        .send()
        .map_err(|e| format!("Failed to download file: {}", e))?;

        let mut file = fs::File::create(destination)
        .map_err(|e| format!("Failed to create file: {}", e))?;

        let content = response.bytes()
        .map_err(|e| format!("Failed to read response bytes: {}", e))?;

        file.write_all(&content)
        .map_err(|e| format!("Failed to write to file: {}", e))?;

        println!("{}Download complete!{}", COLOR_GREEN, COLOR_RESET);
        return Ok(());
    }
}

// Install Battle.net
fn install_battlenet(wine_path: &str, app_paths: &AppPaths) -> Result<(), String> {
    println!("{}Preparing to install Battle.net...{}", COLOR_BLUE, COLOR_RESET);

    // Create battlenet directory if it doesn't exist
    let battlenet_dir = app_paths.home_dir.join(".battlenet");
    fs::create_dir_all(&battlenet_dir)
    .map_err(|e| format!("Failed to create Battle.net directory: {}", e))?;

    let installer_url = "https://downloader.battle.net/download/getInstaller?os=win&installer=Battle.net-Setup.exe";
    download_file(installer_url, &app_paths.battlenet_installer)?;

    // Make installer executable
    if let Err(e) = fs::set_permissions(&app_paths.battlenet_installer, fs::Permissions::from_mode(0o755)) {
        println!("{}Warning: Could not make installer executable: {}{}", COLOR_YELLOW, e, COLOR_RESET);
    }

    // Determine wine prefix
    let wine_prefix = app_paths.home_dir.join(".wine");

    // Prompt for install directory
    println!("{}Where do you want to install Battle.net?{}", COLOR_BLUE, COLOR_RESET);
    let default_install_dir = app_paths.home_dir.join("Games/Battle.net").to_string_lossy().to_string();
    println!("Installation directory (Default: {}): ", default_install_dir);

    io::stdout().flush().unwrap();
    let mut install_dir = String::new();
    io::stdin().read_line(&mut install_dir).unwrap();
    install_dir = install_dir.trim().to_string();

    let install_dir = if install_dir.is_empty() {
        default_install_dir
    } else {
        install_dir
    };

    // Create the directory if it doesn't exist
    fs::create_dir_all(&install_dir).map_err(|e| format!("Failed to create installation directory: {}", e))?;

    println!("\n{}Running Battle.net installer in silent mode...{}", COLOR_BLUE, COLOR_RESET);

    // Use the exact command that the user confirmed works
    let mut command = Command::new(wine_path);
    command
    .env("WINEPREFIX", wine_prefix.to_string_lossy().to_string())
    .env("WINEDEBUG", "-all")  // Suppress all Wine debug messages
    .env("MANGOHUD", "0")      // Disable MangoHud
    .env("DISABLE_MANGOHUD", "1") // Another way to disable MangoHud
    .env("WINEDLLOVERRIDES", "mscoree,mshtml=") // Disable browser component
    .env("DISPLAY", ":99")     // Use a fake display to hide GUI
    .env("DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1", "1") // Disable AMD layers
    .arg(&app_paths.battlenet_installer)
    .arg("--lang=enUS")
    .arg("--installpath=\"C:\\Program Files (x86)\\Battle.net\"")
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null());

    let silent_status = command.status().map_err(|e| format!("Failed to execute wine command: {}", e))?;
    let install_status = silent_status.code().unwrap_or(1);

    if install_status != 0 {
        println!("{}Silent install failed. Falling back to interactive mode...{}",
                 COLOR_RED, COLOR_RESET);
        println!("\n{}Running Battle.net installer interactively...{}", COLOR_BLUE, COLOR_RESET);
        println!("{}Please follow the installation instructions in the installer window.{}", COLOR_YELLOW, COLOR_RESET);

        // For interactive mode
        let mut interactive_command = Command::new(wine_path);
        interactive_command
        .env("WINEPREFIX", wine_prefix.to_string_lossy().to_string())
        .env("WINEDEBUG", "-all")  // Suppress all Wine debug messages
        .env("MANGOHUD", "0")      // Disable MangoHud
        .env("DISABLE_MANGOHUD", "1") // Another way to disable MangoHud
        .env("DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1", "1") // Try to disable some AMD layers
        .arg(&app_paths.battlenet_installer)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

        let interactive_status = interactive_command.status()
        .map_err(|e| format!("Failed to execute wine command: {}", e))?
        .code()
        .unwrap_or(1);

        if interactive_status != 0 {
            println!("{}The Battle.net installer encountered an error (status code: {}).{}",
                     COLOR_RED, interactive_status, COLOR_RESET);

            print!("Would you like to continue anyway? (yes/no)\n> ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();

            if input.trim().to_lowercase() != "yes" && input.trim().to_lowercase() != "y" {
                return Err("Operation cancelled based on installer error.".to_string());
            }
        }
    }

    // Run wineserver -k with suppressed output
    println!("{}Running wineserver -k to clean up...{}", COLOR_YELLOW, COLOR_RESET);
    let _ = Command::new("wineserver")
    .arg("-k")
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .status();
    thread::sleep(Duration::from_secs(1));

    // Look for the actual Battle.net installation location
    let possible_locations = [
        app_paths.home_dir.join(".wine/drive_c/Program Files/Battle.net"),
        app_paths.home_dir.join(".wine/drive_c/Program Files (x86)/Battle.net"),
        app_paths.home_dir.join(".wine/drive_c/Games/Battle.net"),
        app_paths.home_dir.join(".wine/drive_c/Blizzard/Battle.net"),
    ];

    let mut found_location = None;
    for location in &possible_locations {
        if location.exists() && location.is_dir() {
            found_location = Some(location);
            break;
        }
    }

    match found_location {
        Some(source_path) => {
            println!("{}Found Battle.net installation at: {}{}", COLOR_GREEN, source_path.display(), COLOR_RESET);

            // Copy files from the Wine C: drive to the user's specified location
            if source_path.to_string_lossy() != install_dir {
                println!("{}Copying Battle.net files to {}...{}", COLOR_BLUE, install_dir, COLOR_RESET);

                // Copy all files recursively
                match copy_dir_recursive(&source_path, &PathBuf::from(&install_dir)) {
                    Ok(_) => {
                        println!("{}Files copied successfully.{}", COLOR_GREEN, COLOR_RESET);

                        println!("{}Would you like to delete the original files in Wine's C: drive? (yes/no){}",
                                 COLOR_YELLOW, COLOR_RESET);

                        print!("> ");
                        io::stdout().flush().unwrap();

                        let mut delete_choice = String::new();
                        io::stdin().read_line(&mut delete_choice).unwrap();

                        if delete_choice.trim().to_lowercase() == "yes" || delete_choice.trim().to_lowercase() == "y" {
                            match fs::remove_dir_all(source_path) {
                                Ok(_) => println!("{}Original directory deleted.{}", COLOR_GREEN, COLOR_RESET),
                                Err(e) => println!("{}Error deleting original directory: {}{}", COLOR_RED, e, COLOR_RESET)
                            }
                        }
                    },
                    Err(e) => println!("{}Error copying files: {}{}", COLOR_RED, e, COLOR_RESET)
                }
            }
        },
        None => {
            println!("{}Warning: Could not find Battle.net installation directory in Wine C: drive.{}",
                     COLOR_YELLOW, COLOR_RESET);
            println!("{}Please check if Battle.net was installed correctly.{}", COLOR_YELLOW, COLOR_RESET);
        }
    }

    println!("{}Battle.net installation completed.{}", COLOR_GREEN, COLOR_RESET);
    println!("{}Installed to: {}{}", COLOR_GREEN, install_dir, COLOR_RESET);

    // Steam integration instructions - simplified
    println!("\n{}=== How to Add Battle.net to Steam ==={}", COLOR_BLUE, COLOR_RESET);
    println!("{}1. Open Steam and click on 'Add a Game' in the bottom-left corner{}", COLOR_GREEN, COLOR_RESET);
    println!("{}2. Select 'Add a Non-Steam Game...'{}", COLOR_GREEN, COLOR_RESET);
    println!("{}3. Click 'BROWSE' and navigate to your Battle.net installation folder:{}", COLOR_GREEN, COLOR_RESET);
    println!("   {}{}", COLOR_YELLOW, install_dir);
    println!("{}4. Select the 'Battle.net.exe' file and click 'Open'{}", COLOR_GREEN, COLOR_RESET);
    println!("{}5. Click 'Add Selected Program'{}", COLOR_GREEN, COLOR_RESET);
    println!("{}6. Battle.net is now ready to use in Steam!{}\n", COLOR_GREEN, COLOR_RESET);

    Ok(())
}

// Install HoYoPlay
fn install_hoyoplay(wine_path: &str, app_paths: &AppPaths) -> Result<(), String> {
    println!("{}Preparing to install HoYoPlay...{}", COLOR_BLUE, COLOR_RESET);

    // Create hoyoplay directory if it doesn't exist
    let hoyoplay_dir = app_paths.home_dir.join(".hoyoplay");
    fs::create_dir_all(&hoyoplay_dir)
    .map_err(|e| format!("Failed to create HoYoPlay directory: {}", e))?;

    let installer_url = "https://download-porter.hoyoverse.com/download-porter/2025/02/21/VYTpXlbWo8_1.4.5.222_1_0_hyp_hoyoverse_prod_202502081529_XFGRLkBk.exe?trace_key=HoYoPlay_install_ua_5ca9c7368584";
    download_file(installer_url, &app_paths.hoyoplay_installer)?;

    // Make installer executable
    if let Err(e) = fs::set_permissions(&app_paths.hoyoplay_installer, fs::Permissions::from_mode(0o755)) {
        println!("{}Warning: Could not make installer executable: {}{}", COLOR_YELLOW, e, COLOR_RESET);
    }

    // Determine wine prefix
    let wine_prefix = app_paths.home_dir.join(".wine");

    // Prompt for install directory
    println!("{}Where do you want to install HoYoPlay?{}", COLOR_BLUE, COLOR_RESET);
    let default_hoyo_dest = app_paths.home_dir.join("Games/HoYoPlay").to_string_lossy().to_string();
    println!("Destination folder (Default: {}): ", default_hoyo_dest);

    io::stdout().flush().unwrap();
    let mut hoyo_dest = String::new();
    io::stdin().read_line(&mut hoyo_dest).unwrap();
    hoyo_dest = hoyo_dest.trim().to_string();

    let hoyo_dest_path = if hoyo_dest.is_empty() {
        default_hoyo_dest
    } else {
        hoyo_dest
    };

    // Create destination directory if it doesn't exist
    fs::create_dir_all(&hoyo_dest_path)
    .map_err(|e| format!("Failed to create directory: {}", e))?;

    println!("\n{}Running HoYoPlay installer...{}", COLOR_BLUE, COLOR_RESET);

    // Create command with suppressed output and environment variables similar to Battle.net
    let mut command = Command::new(wine_path);
    command
    .env("WINEPREFIX", wine_prefix.to_string_lossy().to_string())
    .env("WINEDEBUG", "-all")  // Suppress all Wine debug messages
    .env("MANGOHUD", "0")      // Disable MangoHud
    .env("DISABLE_MANGOHUD", "1") // Another way to disable MangoHud
    .env("WINEDLLOVERRIDES", "mscoree,mshtml=") // Disable browser component
    .env("DISPLAY", ":99")     // Use a fake display to hide GUI
    .env("DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1", "1") // Try to disable some AMD layers
    .arg(&app_paths.hoyoplay_installer)
    .stdout(std::process::Stdio::null()) // Redirect stdout to null
    .stderr(std::process::Stdio::null()); // Redirect stderr to null

    // Run the HoYoPlay installer
    let install_status = command.status()
    .map_err(|e| format!("Failed to execute wine command: {}", e))?
    .code()
    .unwrap_or(1);

    // Run wineserver -k with suppressed output
    println!("{}Running wineserver -k to clean up...{}", COLOR_YELLOW, COLOR_RESET);
    let _ = Command::new("wineserver")
    .arg("-k")
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .status();
    thread::sleep(Duration::from_secs(1));

    if install_status != 0 {
        println!("{}The HoYoPlay installer encountered an error (status code: {}).{}",
                 COLOR_RED, install_status, COLOR_RESET);

        print!("Would you like to continue anyway? (yes/no)\n> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if input.trim().to_lowercase() != "yes" && input.trim().to_lowercase() != "y" {
            return Err("Operation cancelled based on HoYoPlay installer error.".to_string());
        }
    }

    println!("{}HoYoPlay installation finished. Installed to default Wine C: drive.{}", COLOR_GREEN, COLOR_RESET);

    // Copy files from Wine C: drive to the destination directory
    let hoyo_src = app_paths.home_dir.join(".wine/drive_c/Program Files/HoYoPlay");

    if hoyo_src.exists() && hoyo_src.is_dir() {
        println!("{}Copying HoYoPlay files to {}...{}", COLOR_BLUE, hoyo_dest_path, COLOR_RESET);

        // Copy all files recursively
        copy_dir_recursive(&hoyo_src, &PathBuf::from(&hoyo_dest_path))
        .map_err(|e| format!("Failed to copy files: {}", e))?;

        println!("{}Files copied successfully.{}", COLOR_GREEN, COLOR_RESET);

        println!("{}Deleting original HoYoPlay directory in .wine...{}", COLOR_YELLOW, COLOR_RESET);
        fs::remove_dir_all(&hoyo_src)
        .map_err(|e| format!("Failed to delete directory: {}", e))?;

        println!("{}Original directory deleted.{}", COLOR_GREEN, COLOR_RESET);
    } else {
        println!("{}HoYoPlay directory not found in .wine!{}", COLOR_RED, COLOR_RESET);
    }

    // Steam integration instructions - simplified
    println!("\n{}=== How to Add HoYoPlay to Steam ==={}", COLOR_BLUE, COLOR_RESET);
    println!("{}1. Open Steam and click on 'Add a Game' in the bottom-left corner{}", COLOR_GREEN, COLOR_RESET);
    println!("{}2. Select 'Add a Non-Steam Game...'{}", COLOR_GREEN, COLOR_RESET);
    println!("{}3. Click 'BROWSE' and navigate to your HoYoPlay installation folder:{}", COLOR_GREEN, COLOR_RESET);
    println!("   {}{}", COLOR_YELLOW, hoyo_dest_path);
    println!("{}4. Select the 'HoYoPlay.exe' file and click 'Open'{}", COLOR_GREEN, COLOR_RESET);
    println!("{}5. Click 'Add Selected Program'{}", COLOR_GREEN, COLOR_RESET);
    println!("{}6. HoYoPlay is now ready to use in Steam!{}\n", COLOR_GREEN, COLOR_RESET);

    // Important note about running HoYoPlay once before post-setup
    println!("{}IMPORTANT: You should launch HoYoPlay once from Steam before running{}", COLOR_YELLOW, COLOR_RESET);
    println!("{}the 'Run HoYoPlay Post-Setup' option from this installer.{}", COLOR_YELLOW, COLOR_RESET);
    println!("{}This ensures all necessary files and settings are properly initialized.{}\n", COLOR_YELLOW, COLOR_RESET);

    Ok(())
}

// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

// Check if protontricks is installed
fn check_protontricks() -> bool {
    Command::new("which")
    .arg("protontricks")
    .status()
    .map(|status| status.success())
    .unwrap_or(false)
}

// List non-Steam games using protontricks
fn list_nonsteam_games() -> Result<Vec<String>, String> {
    let output = Command::new("protontricks")
    .arg("-l")
    .output()
    .map_err(|e| format!("Failed to execute protontricks: {}", e))?;

    if !output.status.success() {
        return Err("protontricks -l command failed".to_string());
    }

    let output_str = str::from_utf8(&output.stdout)
    .map_err(|e| format!("Invalid UTF-8 in protontricks output: {}", e))?;

    let mut games: Vec<String> = Vec::new();

    for line in output_str.lines() {
        if line.contains("Non-Steam shortcut:") {
            games.push(line.to_string());
        }
    }

    Ok(games)
}

// Extract App ID from a protontricks game line
fn extract_appid(line: &str) -> Option<String> {
    let re = Regex::new(r"\(([0-9]+)\)$").unwrap();
    re.captures(line).and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
}

// Find Steam library folders
fn find_steam_libraries() -> Result<Vec<PathBuf>, String> {
    let home_dir = dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;
    let steam_root = home_dir.join(".steam/steam");
    let library_vdf = steam_root.join("steamapps/libraryfolders.vdf");

    if !library_vdf.exists() {
        return Err(format!("Could not find libraryfolders.vdf at {}", library_vdf.display()));
    }

    let mut libraries = vec![steam_root];

    // Parse libraryfolders.vdf to find additional library paths
    let vdf_content = fs::read_to_string(&library_vdf)
    .map_err(|e| format!("Failed to read libraryfolders.vdf: {}", e))?;

    // Simple regex to extract paths from VDF
    let re = Regex::new(r#""path"\s*"([^"]+)""#).unwrap();
    for cap in re.captures_iter(&vdf_content) {
        if let Some(path_match) = cap.get(1) {
            let path = PathBuf::from(path_match.as_str().replace("\\\\", "/"));
            libraries.push(path);
        }
    }

    Ok(libraries)
}

// Find the compatdata prefix for an App ID
fn find_prefix_path(app_id: &str, libraries: &[PathBuf]) -> Option<PathBuf> {
    for lib in libraries {
        let compatdata_path = lib.join(format!("steamapps/compatdata/{}/pfx", app_id));
        if compatdata_path.exists() && compatdata_path.is_dir() {
            return Some(compatdata_path);
        }
    }
    None
}

// Set up symlink to Linux root in the Wine prefix
fn setup_linux_root_symlink(prefix_path: &Path) -> Result<(), String> {
    let linux_root_link = prefix_path.join("drive_c/Linux Root");

    if linux_root_link.exists() {
        println!("{}Symlink or folder 'Linux Root' already exists in drive_c. Skipping symlink creation.{}",
                 COLOR_YELLOW, COLOR_RESET);
    } else {
        let status = Command::new("ln")
        .arg("-s")
        .arg("/")
        .arg(&linux_root_link)
        .status()
        .map_err(|e| format!("Failed to create symlink: {}", e))?;

        if status.success() {
            println!("{}Symlinked / to {}{}", COLOR_GREEN, linux_root_link.display(), COLOR_RESET);
        } else {
            return Err("Failed to create symlink.".to_string());
        }
    }

    Ok(())
}

// Set registry key to remove window decorations
fn remove_window_decorations(prefix_path: &Path) -> Result<(), String> {
    // Determine which Wine binary to use
    let wine_bin = if Command::new("which").arg("wine64").status().map(|s| s.success()).unwrap_or(false) {
        "wine64"
    } else {
        "wine"
    };

    println!("{}Setting registry key to remove window decorations...{}", COLOR_YELLOW, COLOR_RESET);

    let status = Command::new(wine_bin)
    .env("WINEPREFIX", prefix_path)
    .args(&["reg", "add", "HKCU\\Software\\Wine\\X11 Driver", "/v", "Decorated", "/t", "REG_SZ", "/d", "N", "/f"])
    .status()
    .map_err(|e| format!("Failed to execute Wine registry command: {}", e))?;

    if status.success() {
        println!("{}Window decorations disabled for prefix {}.{}", COLOR_GREEN, prefix_path.display(), COLOR_RESET);
        Ok(())
    } else {
        Err("Failed to set registry key.".to_string())
    }
}

// Run HoYoPlay post-setup
fn run_hoyoplay_postsetup() -> Result<(), String> {
    if !check_protontricks() {
        return Err("protontricks is not installed. Please install it first.".to_string());
    }

    println!("{}Detecting non-Steam games (protontricks -l):{}", COLOR_BLUE, COLOR_RESET);
    let games = list_nonsteam_games()?;

    if games.is_empty() {
        return Err("No non-Steam games found!".to_string());
    }

    println!("{}Select the HoYoPlay entry from the list below:{}", COLOR_YELLOW, COLOR_RESET);
    for (i, game) in games.iter().enumerate() {
        println!("{:2}) {}", i+1, game);
    }

    print!("> ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    let game_index = match input.trim().parse::<usize>() {
        Ok(i) if i >= 1 && i <= games.len() => i - 1,
        _ => return Err("Invalid selection.".to_string()),
    };

    let selected_line = &games[game_index];
    let app_id = extract_appid(selected_line)
    .ok_or_else(|| "Could not extract App ID.".to_string())?;

    let libraries = find_steam_libraries()?;

    let prefix_path = find_prefix_path(&app_id, &libraries)
    .ok_or_else(|| format!("Could not find compatdata prefix for App ID {} in any Steam library.", app_id))?;

    println!("{}Found prefix: {}{}", COLOR_GREEN, prefix_path.display(), COLOR_RESET);

    setup_linux_root_symlink(&prefix_path)?;

    println!("{}You can now access your Linux filesystem from within the game installer by navigating to C:\\Linux Root in the file dialog (look under 'Computer' > 'C:').{}",
             COLOR_GREEN, COLOR_RESET);

    remove_window_decorations(&prefix_path)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}===== Game Launcher Installer ====={}", COLOR_BLUE, COLOR_RESET);

    // Find system wine before showing menu
    let wine_path = match find_system_wine() {
        Some(path) => path,
        None => {
            println!("{}Please install wine and try again.{}", COLOR_RED, COLOR_RESET);
            std::process::exit(1);
        }
    };

    println!("{}Using system wine: {}{}", COLOR_GREEN, wine_path, COLOR_RESET);

    // Setup application paths
    let home_dir = dirs::home_dir().expect("Could not determine home directory");
    let app_paths = AppPaths {
        home_dir: home_dir.clone(),
        battlenet_installer: home_dir.join(".battlenet/Battle.net-Setup.exe"),
        hoyoplay_installer: home_dir.join(".hoyoplay/HoYoPlay-Setup.exe"),
    };

    // Show main menu
    loop {
        println!("What would you like to do?");
        println!("1) Install Battle.net");
        println!("2) Install HoYoPlay");
        println!("3) Run HoYoPlay Post-Setup (removes window decorations)");
        println!("4) Exit");

        print!("Enter your choice [1-4]: ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();

        match choice.trim() {
            "1" => {
                if let Err(e) = install_battlenet(&wine_path, &app_paths) {
                    println!("{}Error: {}{}", COLOR_RED, e, COLOR_RESET);
                    return Err(e.into());
                }
                println!("{}Operation completed successfully.{}", COLOR_GREEN, COLOR_RESET);
                break;
            },
            "2" => {
                if let Err(e) = install_hoyoplay(&wine_path, &app_paths) {
                    println!("{}Error: {}{}", COLOR_RED, e, COLOR_RESET);
                    return Err(e.into());
                }
                println!("{}Operation completed successfully.{}", COLOR_GREEN, COLOR_RESET);
                break;
            },
            "3" => {
                println!("\n{}===== HoYoPlay Post-Setup ====={}", COLOR_BLUE, COLOR_RESET);
                println!("{}Before running this tool, make sure you have:{}", COLOR_YELLOW, COLOR_RESET);
                println!("{}1. Added HoYoPlay to Steam using the instructions provided after installation{}", COLOR_YELLOW, COLOR_RESET);
                println!("{}2. Launched HoYoPlay from Steam at least once{}", COLOR_YELLOW, COLOR_RESET);
                println!("{}3. Created a non-Steam shortcut in Steam for the game you want to play{}", COLOR_YELLOW, COLOR_RESET);
                println!("{}This tool will remove window decorations to give a cleaner gaming experience.{}\n", COLOR_YELLOW, COLOR_RESET);

                print!("Do you want to continue? (yes/no): ");
                io::stdout().flush().unwrap();

                let mut confirm = String::new();
                io::stdin().read_line(&mut confirm).unwrap();

                if confirm.trim().to_lowercase() == "yes" || confirm.trim().to_lowercase() == "y" {
                    if let Err(e) = run_hoyoplay_postsetup() {
                        println!("{}Error: {}{}", COLOR_RED, e, COLOR_RESET);
                        return Err(e.into());
                    }
                    println!("{}Operation completed successfully.{}", COLOR_GREEN, COLOR_RESET);
                } else {
                    println!("{}Post-setup cancelled.{}", COLOR_YELLOW, COLOR_RESET);
                }
                break;
            },
            "4" => {
                println!("{}Exiting.{}", COLOR_YELLOW, COLOR_RESET);
                break;
            },
            _ => {
                println!("{}Invalid choice. Please enter a number between 1 and 4.{}", COLOR_RED, COLOR_RESET);
            }
        }
    }

    Ok(())
}
