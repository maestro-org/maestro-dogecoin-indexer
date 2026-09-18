use std::{ops::Deref, sync::RwLock};

use actix_web::{
    middleware,
    web::{self, Data, Json},
    App, HttpRequest, HttpServer,
};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use gasket::framework::*;
use serde::Serialize;
use tokio::{task::JoinHandle, try_join};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::model::HealthEvent;

pub type RollUpstreamPort = gasket::messaging::tokio::InputPort<HealthEvent>;

#[derive(Debug)]
pub enum HealthUnit {
    Roll(HealthEvent),
}

#[derive(Stage)]
#[stage(name = "health", unit = "HealthUnit", worker = "Worker")]
pub struct Stage {
    endpoint: String,
    node_rpc: String,
    node_rpc_auth: Auth,
    pub roll_upstream: RollUpstreamPort,
}

impl Stage {
    pub fn new(endpoint: String, node_rpc: String, node_rpc_auth: Auth) -> Self {
        Self {
            endpoint,
            node_rpc,
            node_rpc_auth,
            roll_upstream: Default::default(),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct Tip {
    height: u64,
    hash: String,
}

#[derive(Default, Serialize, Clone)]
pub struct StageHealth {
    roll_stage: Option<Tip>,
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct HealthResponse {
    upstream: Option<Tip>,
    roll_stage: Option<Tip>,
}

pub struct Worker {
    health: web::Data<RwLock<StageHealth>>,
    handle: JoinHandle<()>,
}

#[async_trait::async_trait(?Send)]
impl gasket::framework::Worker<Stage> for Worker {
    async fn bootstrap(stage: &Stage) -> Result<Self, WorkerError> {
        let health_lock = web::Data::new(RwLock::new(Default::default()));
        let peer_rpc =
            web::Data::new(Client::new(&stage.node_rpc, stage.node_rpc_auth.clone()).or_panic()?);

        let health_lock_clone = health_lock.clone();
        let peer_rpc_clone = peer_rpc.clone();

        let server = HttpServer::new(move || {
            App::new()
                .app_data(health_lock.clone())
                .app_data(peer_rpc_clone.clone())
                .wrap(middleware::Logger::default())
                .service(web::resource("/health").to(health))
        })
        .bind(stage.endpoint.clone())
        .or_panic()?
        .run();

        let handle = tokio::spawn(async {
            info!("health server spawned");
            let res = try_join!(server);
            warn!(?res, "health handle ended")
        });

        let worker = Worker {
            health: health_lock_clone,
            handle,
        };

        Ok(worker)
    }

    async fn schedule(
        &mut self,
        stage: &mut Stage,
    ) -> Result<WorkSchedule<HealthUnit>, WorkerError> {
        if self.handle.is_finished() {
            warn!("health server finished, restarting");
            return Err(WorkerError::Restart);
        }

        let msg = stage.roll_upstream.recv().await.or_panic()?;
        Ok(WorkSchedule::Unit(HealthUnit::Roll(msg.payload)))
    }

    async fn execute(&mut self, unit: &HealthUnit, _stage: &mut Stage) -> Result<(), WorkerError> {
        match unit {
            HealthUnit::Roll(x) => {
                let mut health_lock = self.health.write().or_restart()?;

                let (height, hash) = match x {
                    HealthEvent::RollForward(hi, ha) => (*hi, ha),
                    HealthEvent::RollBack(hi, ha) => (*hi, ha),
                };

                if height % 20 == 0 {
                    info!("received health roll message: {x:?}");
                }

                health_lock.roll_stage = Some(Tip {
                    height,
                    hash: hex::encode(AsRef::<[u8; 32]>::as_ref(hash)),
                });
            }
        }

        Ok(())
    }

    async fn teardown(&mut self) -> Result<(), WorkerError> {
        self.handle.abort();

        Ok(())
    }
}

/// endpoint handler
async fn health(
    health: Data<RwLock<StageHealth>>,
    rpc: Data<Client>,
    _: HttpRequest,
) -> Json<HealthResponse> {
    let request_id = Uuid::new_v4();

    debug!(
        request_id = request_id.to_string(),
        "received health request"
    );

    let health = match health.read() {
        Ok(h) => Some(h.deref().clone()),
        Err(e) => {
            error!(?e, "error getting health lock");
            None
        }
    };

    let upstream = match rpc.get_chain_tips() {
        Ok(t) => t.into_iter().max_by_key(|x| x.height).map(|x| Tip {
            height: x.height,
            hash: hex::encode(AsRef::<[u8; 32]>::as_ref(&x.hash)),
        }),
        Err(e) => {
            error!(?e, "error getting upstream tips");
            None
        }
    };

    let out = HealthResponse {
        upstream,
        roll_stage: health.map(|x| x.roll_stage).flatten(),
    };

    debug!(
        request_id = request_id.to_string(),
        ?out,
        "returning health response"
    );

    Json(out)
}
