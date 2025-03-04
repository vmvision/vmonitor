use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo};
use serde::{Deserialize, Serialize};
use sysinfo::{Disks, Networks, RefreshKind, System};

#[derive(Deserialize, Serialize, Debug)]
pub struct VMInfo {
    os: String,
    arch: String,
    kernel: String,
    hostname: String,
    cpu: String,
    memory: u64,
    uptime: u64,
    disk: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReportData {
    pub uptime: u64,
    pub system: SystemInfo,
    pub network: NetworkInfo,
    pub disk: DiskInfo,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SystemInfo {
    cpu_usage: f32,
    memory_used: u64,
    memory_total: u64,
    process_count: u32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct NetworkInfo {
    download_traffic: u64,
    upload_traffic: u64,
    tcp_count: u32,
    udp_count: u32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct DiskInfo {
    space_used: u64,
    space_total: u64,
}

pub fn collect_vm_info(system: &mut System, disks: &mut Disks) -> VMInfo {
    let cpu = if let Some(cpu) = system.cpus().first() {
        format!(
            "{} ({:.2} GHz)",
            cpu.brand(),
            cpu.frequency() as f64 / 1000.0
        )
    } else {
        "Unknown".to_string()
    };
    VMInfo {
        os: System::name().unwrap_or_else(|| "Unknown".to_string()),
        arch: std::env::consts::ARCH.to_string(),
        kernel: System::kernel_version().unwrap_or_else(|| "Unknown".to_string()),
        hostname: System::host_name().unwrap_or_else(|| "Unknown".to_string()),
        cpu,
        memory: system.total_memory(),
        uptime: System::uptime(),
        disk: disks.list().iter().map(|d| d.total_space()).sum(),
    }
}

pub fn collect_system_info(system: &mut System) -> SystemInfo {
    system.refresh_specifics(RefreshKind::everything());

    SystemInfo {
        cpu_usage: system.global_cpu_usage(),
        memory_used: system.used_memory(),
        memory_total: system.total_memory(),
        process_count: system.processes().len() as u32,
    }
}

fn collect_socket_number() -> (u32, u32) {
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;

    let sockets = match get_sockets_info(af_flags, proto_flags) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to get socket info: {}", e);
            return (0, 0);
        }
    };

    let mut tcp_count = 0;
    let mut udp_count = 0;

    for socket in sockets {
        match socket.protocol_socket_info {
            ProtocolSocketInfo::Tcp(_) => {
                tcp_count += 1;
            }
            ProtocolSocketInfo::Udp(_) => {
                udp_count += 1;
            }
        }
    }

    (tcp_count, udp_count)
}

pub fn collect_network_info(networks: &mut Networks) -> NetworkInfo {
    networks.refresh(true);

    let mut download_traffic = 0;
    let mut upload_traffic = 0;

    for network in networks.list().values() {
        download_traffic += network.total_received();
        upload_traffic += network.total_transmitted();
    }

    let (tcp_count, udp_count) = collect_socket_number();

    NetworkInfo {
        download_traffic,
        upload_traffic,
        tcp_count,
        udp_count,
    }
}

pub fn collect_disk_info(disks: &mut Disks) -> DiskInfo {
    disks.refresh(true);

    let mut space_used = 0;
    let mut space_total = 0;

    for disk in disks.list() {
        space_total += disk.total_space();
        space_used += disk.total_space() - disk.available_space();
    }

    DiskInfo {
        space_used,
        space_total,
    }
}
