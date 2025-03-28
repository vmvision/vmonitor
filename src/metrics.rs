use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo};
use serde::{Deserialize, Serialize};
use sysinfo::{Disks, Networks, RefreshKind, System};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VMInfo {
    os: String,
    os_version: String,
    arch: String,
    platform: String,
    platform_version: String,
    kernel: String,
    hostname: String,
    cpu: Vec<String>,
    memory: u64,
    uptime: u64,
    disk: u64,
    version: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ReportData {
    pub uptime: u64,
    pub system: SystemInfo,
    pub network: NetworkInfo,
    pub disk: DiskInfo,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SystemLoadAvg {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub cpu_usage: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub process_count: u32,
    pub load_avg: SystemLoadAvg,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInfo {
    download_traffic: u64,
    upload_traffic: u64,
    tcp_count: u32,
    udp_count: u32,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    space_used: u64,
    space_total: u64,
    read: u64,
    write: u64,
}

pub struct Metrics {
    pub system: System,
    pub networks: Networks,
    pub disks: Disks,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            system: System::new_all(),
            networks: Networks::new(),
            disks: Disks::new(),
        }
    }

    pub fn collect_vm_info(&mut self) -> VMInfo {
        let cpus: Vec<String> = self
            .system
            .cpus()
            .iter()
            .map(|cpu| {
                format!(
                    "{} ({:.2} GHz)",
                    cpu.brand(),
                    cpu.frequency() as f64 / 1000.0
                )
            })
            .collect();

        let os_info = os_info::get();

        VMInfo {
            os: os_info.os_type().to_string(),
            os_version: os_info.version().to_string(),
            arch: std::env::consts::ARCH.to_string(),
            platform: std::env::consts::OS.to_string(),
            platform_version: os_info
                .edition()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Unknown".to_string()),
            kernel: System::kernel_version().unwrap_or_else(|| "Unknown".to_string()),
            hostname: System::host_name().unwrap_or_else(|| "Unknown".to_string()),
            cpu: if cpus.is_empty() {
                vec!["Unknown".to_string()]
            } else {
                cpus
            },
            memory: self.system.total_memory(),
            disk: self.disks.list().iter().map(|d| d.total_space()).sum(),
            uptime: System::uptime(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub async fn collet_metrics(&mut self) -> ReportData {
        let system_data = self.collect_system_info();
        let network_data = self.collect_network_info();
        let disk_data = self.collect_disk_info();

        ReportData {
            uptime: System::uptime(),
            system: system_data,
            network: network_data,
            disk: disk_data,
        }
    }

    fn collect_system_info(&mut self) -> SystemInfo {
        self.system.refresh_specifics(RefreshKind::everything());

        let load_avg = System::load_average();

        SystemInfo {
            cpu_usage: self.system.global_cpu_usage(),
            memory_used: self.system.used_memory(),
            memory_total: self.system.total_memory(),
            swap_used: self.system.used_swap(),
            swap_total: self.system.total_swap(),
            process_count: self.system.processes().len() as u32,
            load_avg: SystemLoadAvg {
                one: load_avg.one,
                five: load_avg.five,
                fifteen: load_avg.fifteen,
            },
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

    fn collect_network_info(&mut self) -> NetworkInfo {
        self.networks.refresh(true);

        let mut download_traffic = 0;
        let mut upload_traffic = 0;

        for network in self.networks.list().values() {
            download_traffic += network.total_received();
            upload_traffic += network.total_transmitted();
        }

        let (tcp_count, udp_count) = Metrics::collect_socket_number();

        NetworkInfo {
            download_traffic,
            upload_traffic,
            tcp_count,
            udp_count,
        }
    }

    fn collect_disk_info(&mut self) -> DiskInfo {
        self.disks.refresh(true);

        let mut space_used = 0;
        let mut space_total = 0;
        let mut read = 0;
        let mut write = 0;

        for disk in self.disks.list() {
            space_total += disk.total_space();
            space_used += disk.total_space() - disk.available_space();
            read += disk.usage().total_read_bytes;
            write += disk.usage().total_written_bytes;
        }

        DiskInfo {
            space_used,
            space_total,
            read,
            write,
        }
    }
}
