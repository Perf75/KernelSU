use anyhow::{Ok, Result};
use clap::Parser;
use std::path::Path;

#[cfg(target_os = "android")]
use android_logger::Config;
#[cfg(target_os = "android")]
use log::LevelFilter;

use crate::defs::KSUD_VERBOSE_LOG_FILE;
use crate::{apk_sign, assets, debug, defs, init_event, ksucalls, module, utils};

/// KernelSU userspace cli
#[derive(Parser, Debug)]
#[command(author, version = defs::VERSION_NAME, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, default_value_t = cfg!(debug_assertions))]
    verbose: bool,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Manage KernelSU modules
    Module {
        #[command(subcommand)]
        command: Module,
    },

    /// Trigger `post-fs-data` event
    PostFsData,

    /// Trigger `service` event
    Services,

    /// Trigger `boot-complete` event
    BootCompleted,

    /// SELinux policy Patch tool
    Sepolicy {
        #[command(subcommand)]
        command: Sepolicy,
    },

    /// Manage App Profiles
    Profile {
        #[command(subcommand)]
        command: Profile,
    },
    
    /// For developers
    Debug {
        #[command(subcommand)]
        command: Debug,
    },
}

#[derive(clap::Subcommand, Debug)]
enum Debug {
    /// Set the manager app, kernel CONFIG_KSU_DEBUG should be enabled.
    SetManager {
        /// manager package name
        #[arg(default_value_t = String::from("me.weishu.kernelsu"))]
        apk: String,
    },

    /// Get apk size and hash
    GetSign {
        /// apk path
        apk: String,
    },

    /// Root Shell
    Su {
        /// switch to gloabl mount namespace
        #[arg(short, long, default_value = "false")]
        global_mnt: bool,
    },

    /// Get kernel version
    Version,

    Mount,

    /// For testing
    Test,
}

#[derive(clap::Subcommand, Debug)]
enum Sepolicy {
    /// Patch sepolicy
    Patch {
        /// sepolicy statements
        sepolicy: String,
    },

    /// Apply sepolicy from file
    Apply {
        /// sepolicy file path
        file: String,
    },

    /// Check if sepolicy statement is supported/valid
    Check {
        /// sepolicy statements
        sepolicy: String,
    },
}

#[derive(clap::Subcommand, Debug)]
enum Module {
    /// Install module <ZIP>
    Install {
        /// module zip file path
        zip: String,
    },

    /// Uninstall module <id>
    Uninstall {
        /// module id
        id: String,
    },

    /// enable module <id>
    Enable {
        /// module id
        id: String,
    },

    /// disable module <id>
    Disable {
        // module id
        id: String,
    },

    /// run action for module <id>
    Action {
        // module id
        id: String,
    },

    /// list all modules
    List,
}

#[derive(clap::Subcommand, Debug)]
enum Profile {
    /// get root profile's selinux policy of <package-name>
    GetSepolicy {
        /// package name
        package: String,
    },

    /// set root profile's selinux policy of <package-name> to <profile>
    SetSepolicy {
        /// package name
        package: String,
        /// policy statements
        policy: String,
    },

    /// get template of <id>
    GetTemplate {
        /// template id
        id: String,
    },

    /// set template of <id> to <template string>
    SetTemplate {
        /// template id
        id: String,
        /// template string
        template: String,
    },

    /// delete template of <id>
    DeleteTemplate {
        /// template id
        id: String,
    },

    /// list all templates
    ListTemplates,
}

pub fn run() -> Result<()> {
    #[cfg(target_os = "android")]
    android_logger::init_once(
        Config::default()
            .with_max_level(LevelFilter::Trace) // limit log level
            .with_tag("KernelSU"), // logs will show under mytag tag
    );

    #[cfg(not(target_os = "android"))]
    env_logger::init();

    // the kernel executes su with argv[0] = "su" and replace it with us
    let arg0 = std::env::args().next().unwrap_or_default();
    if arg0 == "su" || arg0 == "/system/bin/su" {
        return crate::su::root_shell();
    }

    let cli = Args::parse();

    if !cli.verbose && !Path::new(KSUD_VERBOSE_LOG_FILE).exists() {
        log::set_max_level(LevelFilter::Info);
    }

    log::info!("command: {:?}", cli.command);

    let result = match cli.command {
        Commands::PostFsData => init_event::on_post_data_fs(),
        Commands::BootCompleted => init_event::on_boot_completed(),

        Commands::Module { command } => {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                utils::switch_mnt_ns(1)?;
                utils::unshare_mnt_ns()?;
            }
            match command {
                Module::Install { zip } => module::install_module(&zip),
                Module::Uninstall { id } => module::uninstall_module(&id),
                Module::Enable { id } => module::enable_module(&id),
                Module::Disable { id } => module::disable_module(&id),
                Module::Action { id } => module::run_action(&id),
                Module::List => module::list_modules(),
            }
        },
        
        Commands::Sepolicy { command } => match command {
            Sepolicy::Patch { sepolicy } => crate::sepolicy::live_patch(&sepolicy),
            Sepolicy::Apply { file } => crate::sepolicy::apply_file(file),
            Sepolicy::Check { sepolicy } => crate::sepolicy::check_rule(&sepolicy),
        },
        
        Commands::Services => init_event::on_services(),
        Commands::Profile { command } => match command {
            Profile::GetSepolicy { package } => crate::profile::get_sepolicy(package),
            Profile::SetSepolicy { package, policy } => {
                crate::profile::set_sepolicy(package, policy)
            }
            Profile::GetTemplate { id } => crate::profile::get_template(id),
            Profile::SetTemplate { id, template } => crate::profile::set_template(id, template),
            Profile::DeleteTemplate { id } => crate::profile::delete_template(id),
            Profile::ListTemplates => crate::profile::list_templates(),
        },

        Commands::Debug { command } => match command {
            Debug::SetManager { apk } => debug::set_manager(&apk),
            Debug::GetSign { apk } => {
                let sign = apk_sign::get_apk_signature(&apk)?;
                println!("size: {:#x}, hash: {}", sign.0, sign.1);
                Ok(())
            }
            Debug::Version => {
                println!("Kernel Version: {}", ksucalls::get_version());
                Ok(())
            }
            Debug::Su { global_mnt } => crate::su::grant_root(global_mnt),
            Debug::Mount => init_event::mount_modules_systemlessly(),
            Debug::Test => assets::ensure_binaries(false),
        },
    };

    if let Err(e) = &result {
        log::error!("Error: {:?}", e);
    }
    result
}
