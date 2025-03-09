use clap::{Arg, Command};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command as ShellCommand;
use confy;

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

fn main() {
    let matches = Command::new("qemuctl")
        .version("0.1.0")
        .author("Your Name")
        .about("QEMU VM Management CLI")
        .subcommand(
            Command::new("create")
                .about("Create a new VM")
                .arg(Arg::new("name").required(true).help("Name of the VM"))
                .arg(Arg::new("memory").short('m').long("memory").value_name("SIZE").help("Amount of RAM (e.g., 4G)").default_value("2G"))
                .arg(Arg::new("cpu").short('c').long("cpu").value_name("CPU").help("CPU configuration (e.g., host)").default_value("host"))
                .arg(Arg::new("disk").short('d').long("disk").value_name("FILE").help("Disk image path").default_value("vm.qcow2"))
                .arg(Arg::new("iso").short('i').long("iso").value_name("FILE").help("Path to ISO for boot")),
        )
        .subcommand(
            Command::new("start")
                .about("Start a VM")
                .arg(Arg::new("name").required(true).help("Name of the VM")),
        )
        .subcommand(
            Command::new("stop")
                .about("Stop a running VM")
                .arg(Arg::new("name").required(true).help("Name of the VM")),
        )
        .subcommand(
            Command::new("list")
                .about("List defined and running VMs"),
        )
        .get_matches();

    let mut config = load_config();

    match matches.subcommand() {
        Some(("create", sub_m)) => {
            let name = sub_m.get_one::<String>("name").unwrap().clone();
            let memory = sub_m.get_one::<String>("memory").unwrap().clone();
            let cpu = sub_m.get_one::<String>("cpu").unwrap().clone();
            let disk = sub_m.get_one::<String>("disk").unwrap().clone();
            let iso = sub_m.get_one::<String>("iso").unwrap_or(&"".to_string()).clone();

            let vm = VMInfo { name: name.clone(), memory, cpu, disk, iso };
            config.vms.insert(name.clone(), vm);
            save_config(&config);

            println!("VM '{}' created and saved.", name);
        }
        Some(("start", sub_m)) => {
            let name = sub_m.get_one::<String>("name").unwrap();
            if let Some(vm) = config.vms.get(name) {
                let cmd = format!(
                    "qemu-system-x86_64 -name {} -m {} -cpu {} -drive file={},format=qcow2 -cdrom {}",
                    vm.name, vm.memory, vm.cpu, vm.disk, vm.iso
                );
                println!("Starting VM: {}", vm.name);
                if let Err(e) = ShellCommand::new("sh").arg("-c").arg(&cmd).status() {
                    eprintln!("Failed to start VM: {}", e);
                }
            } else {
                eprintln!("VM '{}' not found", name);
            }
        }
        Some(("stop", sub_m)) => {
            let name = sub_m.get_one::<String>("name").unwrap();
            println!("Stopping VM: {}", name);
            let cmd = format!("pkill -f 'qemu-system-x86_64.*{}'", name);
            if let Err(e) = ShellCommand::new("sh").arg("-c").arg(&cmd).status() {
                eprintln!("Failed to stop VM: {}", e);
            }
        }
        Some(("list", _)) => {
            println!("Defined VMs:");
            for (name, vm) in &config.vms {
                println!("- {}: {} CPU, {} RAM, Disk: {}", name, vm.cpu, vm.memory, vm.disk);
            }
            println!("\nRunning VMs:");
            if let Err(e) = ShellCommand::new("sh")
                .arg("-c")
                .arg("pgrep -a qemu-system-x86_64")
                .status()
            {
                eprintln!("Failed to list running VMs: {}", e);
            }
        }
        _ => {}
    }
}

