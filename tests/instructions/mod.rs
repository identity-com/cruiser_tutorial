mod create_game;
mod create_profile;
mod join_game;

use cruiser::prelude::*;
use futures::executor::block_on;
use reqwest::Client;
use std::cell::UnsafeCell;
use std::path::Path;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::task::{spawn_blocking, yield_now};
use tokio::time::sleep;

static SETUP: Setup = Setup::new();

/// All tests that need validator access should call this function
/// and call [`TestGuard::drop_self`] when done with the validator.
pub async fn setup_validator() -> TestGuard {
    SETUP.setup().await
}

struct Setup {
    test_count: AtomicIsize,
    program_id: UnsafeCell<Option<Pubkey>>,
    validator: UnsafeCell<Option<Child>>,
}
impl Setup {
    const fn new() -> Self {
        Self {
            test_count: AtomicIsize::new(0),
            program_id: UnsafeCell::new(None),
            validator: UnsafeCell::new(None),
        }
    }

    async fn setup(&'static self) -> TestGuard {
        let mut count = self.test_count.load(Ordering::SeqCst);
        let should_start = loop {
            let should_start = match count {
                -2 => panic!("Validator could not be started"),
                -1 => {
                    // Validator is being killed
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
                0 => true,
                count if count > 0 => false,
                count => panic!("Bad value for count: {}", count),
            };
            assert!(count >= 0);
            match self.test_count.compare_exchange_weak(
                count,
                count + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break should_start,
                Err(new_count) => {
                    count = new_count;
                    yield_now().await;
                }
            }
        };
        if should_start {
            match start_validator().await {
                Ok((program_id, validator)) => unsafe {
                    *self.program_id.get() = Some(program_id);
                    *self.validator.get() = Some(validator);
                },
                Err(e) => {
                    self.test_count.store(-2, Ordering::SeqCst);
                    panic!("Validator could not be started! Error: {}", e);
                }
            }
        }
        let out = TestGuard::new(self);
        let client = Client::new();
        loop {
            if self.test_count.load(Ordering::SeqCst) == -2 {
                panic!("Validator could not be started");
            }
            if client
                .get("http://localhost:8899/health")
                .send()
                .await
                .map_or(false, |res| res.status().is_success())
            {
                break;
            }
            sleep(Duration::from_millis(500)).await;
        }
        out
    }
}
unsafe impl Sync for Setup {}

async fn start_validator() -> Result<(Pubkey, Child), Box<dyn std::error::Error>> {
    let deploy_dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .unwrap()
        .join("deploy");
    let build = Command::new("cargo")
        .env("RUSTFLAGS", "-D warnings")
        .arg("build-bpf")
        .arg("--workspace")
        .spawn()?
        .wait()
        .await?;
    if !build.success() {
        return Err(build.to_string().into());
    }
    let program_id = Keypair::new().pubkey();
    println!("Program ID: `{}`", program_id);

    let mut local_validator = Command::new("solana-test-validator");
    local_validator
        .arg("-r")
        .arg("--bpf-program")
        .arg(program_id.to_string())
        .arg(deploy_dir.join(format!("{}.so", env!("CARGO_PKG_NAME"))))
        .arg("--deactivate-feature")
        .arg("5ekBxc8itEnPv4NzGJtr8BVVQLNMQuLMNQQj7pHoLNZ9") // transaction wide compute cap
        .arg("--deactivate-feature")
        .arg("75m6ysz33AfLA5DDEzWM1obBrnPQRSsdVQ2nRmc8Vuu1") // support account data reallocation
        .arg("--ledger")
        .arg(Path::new(env!("CARGO_TARGET_TMPDIR")).join("test_ledger"));

    println!("Starting local validator...");
    println!("{:?}", local_validator);
    Ok((program_id, local_validator.spawn()?))
}

#[must_use]
pub struct TestGuard {
    setup: &'static Setup,
    rpc: RpcClient,
}
impl TestGuard {
    fn new(setup: &'static Setup) -> Self {
        Self {
            setup,
            rpc: RpcClient::new("http://localhost:8899".to_string()),
        }
    }

    pub fn program_id(&self) -> Pubkey {
        unsafe { (*self.setup.program_id.get()).unwrap() }
    }

    pub fn rpc(&self) -> &RpcClient {
        &self.rpc
    }

    pub async fn drop_self(self) {
        spawn_blocking(move || {
            drop(self);
        })
        .await
        .unwrap();
    }
}
impl Drop for TestGuard {
    fn drop(&mut self) {
        block_on(async {
            let mut count = self.setup.test_count.load(Ordering::SeqCst);
            let should_kill = loop {
                let (replace, should_kill) = match count {
                    count if count < 1 => panic!("`TestGuard` dropped when count less than 1"),
                    1 => (-1, true),
                    count => (count - 1, false),
                };
                match self.setup.test_count.compare_exchange_weak(
                    count,
                    replace,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break should_kill,
                    Err(new_count) => {
                        count = new_count;
                        yield_now().await;
                    }
                }
            };
            if should_kill {
                let mut local = unsafe { (&mut *self.setup.validator.get()).take().unwrap() };
                local.start_kill().unwrap();
                local.wait().await.unwrap();
                assert_eq!(self.setup.test_count.fetch_add(1, Ordering::SeqCst), -1);
                println!("Validator cleaned up properly");
            }
        });
    }
}
