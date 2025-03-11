use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command as ShellCommand;
use std::io::{self, Write};
use std::fs;
use std::path::PathBuf;
use confy;
use home;

#[derive(Debug, Serialize, Deserialize, Default)]
struct VMConfig {
    vms: HashMap<String, VMInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct VMInfo {
    name: String,
    memory: String,
    cpu: String,
    threads: String,  
    disk: String,
    iso: String,
}

const CONFIG_FILE: &str = "qemuctl";

fn load_config() -> VMConfig {
    confy::load(CONFIG_FILE, None).unwrap_or_default()
}

fn save_config(config: &VMConfig) {
    confy::store(CONFIG_FILE, None, config).expect("Failed to save config");
}

fn expand_path(path: &str) -> String {
    if path.starts_with("~") {
        if let Some(home) = home::home_dir() {
            return path.replacen("~", home.to_str().unwrap(), 1);
        }
    }
    path.to_string()
}

fn get_vm_folder() -> String {
    let base_dir = expand_path("~/vms");
    fs::create_dir_all(&base_dir).expect("Failed to create VM directory");
    base_dir
}




fn create_vm(config: &mut VMConfig) {
    let mut input = String::new();

    print!("Enter VM name: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut input).unwrap();
    let name = input.trim().to_string();

    let vm_dir = expand_path(&format!("{}/{}", get_vm_folder(), name));
    fs::create_dir_all(&vm_dir).expect("Failed to create VM directory");

    let disk_path = expand_path(&format!("{}/{}.qcow2", vm_dir, name));

    print!("Memory (default 4G): ");
    io::stdout().flush().unwrap();
    input.clear();
    io::stdin().read_line(&mut input).unwrap();
    let memory = if input.trim().is_empty() { "4G".to_string() } else { input.trim().to_string() };

    print!("Disk size (default 10G): ");
    io::stdout().flush().unwrap();
    input.clear();
    io::stdin().read_line(&mut input).unwrap();
    let disk_size = if input.trim().is_empty() { "10G".to_string() } else { input.trim().to_string() };

    print!("CPU threads (default 1): ");
    io::stdout().flush().unwrap();
    input.clear();
    io::stdin().read_line(&mut input).unwrap();
    let cpu_threads = if input.trim().is_empty() { "1".to_string() } else { input.trim().to_string() };

    print!("ISO path (leave empty if none): ");
    io::stdout().flush().unwrap();
    input.clear();
    io::stdin().read_line(&mut input).unwrap();
    let iso = expand_path(input.trim());

    println!("Creating disk image at {}...", disk_path);
    let _ = ShellCommand::new("qemu-img")
        .arg("create")
        .arg("-f")
        .arg("qcow2")
        .arg(&disk_path)
        .arg(&disk_size)
        .status();

    let vm = VMInfo {
        name: name.clone(),
        memory,
        cpu: "host".to_string(),
        threads: cpu_threads,
        disk: disk_path,
        iso,
    };

    config.vms.insert(name.clone(), vm.clone());
    save_config(config);

    println!("VM '{}' created and saved.", name);

    if !vm.iso.is_empty() {
        print!("Start in GUI or headless mode? (gui/headless): ");
        io::stdout().flush().unwrap();
        input.clear();
        io::stdin().read_line(&mut input).unwrap();

        let mode = input.trim().to_lowercase();
        let display_flag = if mode == "headless" { "-display none" } else { "" };

        // First boot should pass ISO and boot order
        let cmd = format!(
            "setsid qemu-system-x86_64 -name {} -m {} -cpu {} -smp {} -enable-kvm -drive file={},format=qcow2 -cdrom {} -boot order=d {} > /dev/null 2>&1 &",
            vm.name,
            vm.memory,
            vm.cpu,
            vm.threads,
            vm.disk,
            vm.iso,
            display_flag
        );

        println!("Starting VM '{}' in {} mode...", vm.name, mode);
        if let Err(e) = ShellCommand::new("sh").arg("-c").arg(&cmd).spawn() {
            eprintln!("Failed to start VM '{}': {}", vm.name, e);
        }
    }
}

fn start_vm_common(vm: &VMInfo, headless: bool) {
    let display_flag = if headless { "-display none" } else { "" };

    let cmd = format!(
        "setsid qemu-system-x86_64 -name {} -m {} -cpu {} -smp {} -enable-kvm -drive file={},format=qcow2 {} > /dev/null 2>&1 &",
        vm.name,
        vm.memory,
        vm.cpu,
        vm.threads,
        vm.disk,
        display_flag
    );

    println!("Starting VM '{}' in {} mode...", vm.name, if headless { "headless" } else { "GUI" });
    if let Err(e) = ShellCommand::new("sh").arg("-c").arg(&cmd).spawn() {
        eprintln!("Failed to start VM '{}': {}", vm.name, e);
    }
}

fn list_defined_vms(config: &VMConfig) {
    println!("\nDefined VMs:");
    for (name, vm) in &config.vms {
        println!("- {}: {} CPU, {} threads, {} RAM, Disk: {}", name, vm.cpu, vm.threads, vm.memory, vm.disk);
    }
}

fn start_vm(config: &VMConfig) {
    list_defined_vms(config);

    let mut input = String::new();
    print!("Enter VM name to start: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut input).unwrap();
    let name = input.trim();

    if let Some(vm) = config.vms.get(name) {
        print!("Start in GUI or headless mode? (gui/headless): ");
        io::stdout().flush().unwrap();
        input.clear();
        io::stdin().read_line(&mut input).unwrap();

        let mode = input.trim().to_lowercase();
        start_vm_common(vm, mode == "headless");
    } else {
        eprintln!("VM '{}' not found", name);
    }
}

fn stop_vm(config: &VMConfig) {
    list_defined_vms(config);

    let mut input = String::new();
    print!("Enter VM name to stop: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut input).unwrap();
    let name = input.trim();

    if config.vms.contains_key(name) {
        let pattern = format!("qemu-system-x86_64 -name {}", name);
        println!("Stopping VM: {}", name);

        // First, try stopping using pkill (direct command match)
        let pkill_status = ShellCommand::new("pkill")
            .arg("-f")
            .arg(&pattern)
            .status();

        // If pkill fails, try using pgrep + kill (to catch orphaned processes)
        if pkill_status.is_err() {
            if let Ok(output) = ShellCommand::new("pgrep")
                .arg("-f")
                .arg(&pattern)
                .output()
            {
                let pids: Vec<String> = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(|s| s.to_string())
                    .collect();

                for pid in pids {
                    println!("Killing VM process: PID {}", pid);
                    let _ = ShellCommand::new("kill").arg("-9").arg(&pid).status();
                }
            }
        } else {
            println!("VM '{}' stopped.", name);
        }
    } else {
        eprintln!("VM '{}' not found", name);
    }
}

fn delete_vm(config: &mut VMConfig) {
    list_defined_vms(config);

    let mut input = String::new();
    print!("Enter VM name to delete: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut input).unwrap();
    let name = input.trim();

    if let Some(vm) = config.vms.remove(name) {
        // Stop VM first (if it's running)
        let pattern = format!("qemu-system-x86_64 -name {}", name);
        if let Ok(_) = ShellCommand::new("pgrep").arg("-f").arg(&pattern).output() {
            println!("Stopping VM '{}' before deletion...", name);
            stop_vm(config);
        }

        // Remove disk file
        let disk_path = PathBuf::from(expand_path(&vm.disk));
        if disk_path.exists() {
            println!("Deleting disk: {}", disk_path.display());
            if let Err(e) = fs::remove_file(&disk_path) {
                eprintln!("Failed to delete disk file: {}", e);
            }
        }

        // Remove VM folder
        let vm_dir = PathBuf::from(expand_path(&format!("{}/{}", get_vm_folder(), name)));
        if vm_dir.exists() {
            println!("Deleting VM folder: {}", vm_dir.display());
            if let Err(e) = fs::remove_dir_all(&vm_dir) {
                eprintln!("Failed to delete VM folder: {}", e);
            }
        }

        // Save updated config
        save_config(config);

        println!("VM '{}' deleted.", name);
    } else {
        eprintln!("VM '{}' not found", name);
    }
}


fn main() {
    let mut config = load_config();

    loop {
        println!("\n=== QEMU VM Manager ===");
        println!("1. Create VM");
        println!("2. Start VM");
        println!("3. Stop VM");
        println!("4. List VMs");
        println!("5. Delete VM");
        println!("6. Exit");

        print!("\nSelect an option: ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();

        match choice.trim() {
            "1" => create_vm(&mut config),
            "2" => start_vm(&config),
            "3" => stop_vm(&config),
            "4" => list_defined_vms(&config),
            "5" => delete_vm(&mut config),
            "6" => break,
            _ => println!("Invalid choice."),
        }
    }
}

