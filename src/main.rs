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

#[derive(Debug, Serialize, Deserialize)]
struct VMInfo {
    name: String,
    memory: String,
    cpu: String,
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

    let vm = VMInfo { name: name.clone(), memory: memory.clone(), cpu: "host".to_string(), disk: disk_path.clone(), iso: iso.clone() };
    config.vms.insert(name.clone(), vm);
    save_config(config);

    println!("VM '{}' created and saved.", name);

    // Start VM immediately after creation if ISO is specified
    if !iso.is_empty() {
        let cmd = format!(
            "qemu-system-x86_64 -name {} -m {} -cpu host -enable-kvm -drive file={},format=qcow2{}{} &",
            name, memory, disk_path,
            if !iso.is_empty() { format!(" -cdrom {} -boot order=d", iso) } else { "".to_string() },
            " > /dev/null 2>&1"
        );

        println!("Starting VM: {}", name);
        if let Err(e) = ShellCommand::new("sh").arg("-c").arg(&cmd).status() {
            eprintln!("Failed to start VM: {}", e);
        }
    }
}

fn list_defined_vms(config: &VMConfig) {
    println!("\nDefined VMs:");
    for (name, vm) in &config.vms {
        println!("- {}: {} CPU, {} RAM, Disk: {}", name, vm.cpu, vm.memory, vm.disk);
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
        let cmd = format!(
            "qemu-system-x86_64 -name {} -m {} -cpu host -enable-kvm -drive file={},format=qcow2 &> /dev/null &",
            vm.name, vm.memory, vm.disk
        );

        println!("Starting VM: {}", vm.name);
        if let Err(e) = ShellCommand::new("sh").arg("-c").arg(&cmd).status() {
            eprintln!("Failed to start VM: {}", e);
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
        let disk_path = PathBuf::from(expand_path(&vm.disk));
        if disk_path.exists() {
            fs::remove_file(&disk_path).expect("Failed to delete disk file");
        }
        let vm_dir = PathBuf::from(expand_path(&format!("{}/{}", get_vm_folder(), name)));
        if vm_dir.exists() {
            fs::remove_dir_all(&vm_dir).expect("Failed to delete VM directory");
        }
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
            "3" => println!("Stop VM not implemented"),
            "4" => list_defined_vms(&config),
            "5" => delete_vm(&mut config),
            "6" => break,
            _ => println!("Invalid choice. Please try again."),
        }
    }
}

