use std::{
    sync::Arc, 
    marker::{PhantomData, Send, Sync}, 
    net::SocketAddr,
};
use log::debug;
use serde::Serialize;

use tokio::task::JoinHandle;
use http::header::HeaderName;
use warp::{reply, Filter, Rejection, Reply};

use crate::storage::{Storage, Store};
use snarkvm_console_network::Network;

#[derive(Debug, Serialize)]
pub struct Response<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

impl<T: Serialize> Response<T> {
    pub fn new(code: i32, message: String, data: T) -> Self {
        Self { code, message, data }
    }

    pub fn success(data: T) -> Self {
        Self::new(0, String::from("success"), data)
    }

    pub fn json(&self) -> reply::Json {
        reply::json(&self)
    }
}

/// A middleware to include the given item in the handler.
pub fn with<T: Clone + Send>(item: T) -> impl Filter<Extract = (T,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || item.clone())
}


pub struct Server<N: Network, S: Storage<N> + Send + Sync + 'static> {
    store: Arc<Store<N, S>>,
    handles: Vec<Arc<JoinHandle<()>>>,
    _p: PhantomData<N>,
}

impl<N: Network, S: Storage<N> + Send + Sync + 'static> Server<N, S> {
    pub fn start(listen_ip: SocketAddr, store: Store<N, S>) -> Self {
        let mut server = Self { 
            store: Arc::new(store),
            handles: vec![],
            _p: PhantomData,
        };

        server.spawn_server(listen_ip);
        server
    }

     /// Initializes the server.
     fn spawn_server(&mut self, rest_ip: SocketAddr) {
        let cors = warp::cors()
            .allow_any_origin()
            .allow_header(HeaderName::from_static("content-type"))
            .allow_methods(vec!["GET", "POST", "OPTIONS"]);

        // Initialize the routes.
        let routes = self.routes();

        // Add custom logging for each request.
        let custom_log = warp::log::custom(|info| match info.remote_addr() {
            Some(addr) => debug!("Received '{} {}' from '{addr}' ({})", info.method(), info.path(), info.status()),
            None => debug!("Received '{} {}' ({})", info.method(), info.path(), info.status()),
        });

        // Spawn the server.
        self.handles.push(Arc::new(tokio::spawn(async move {
            // Start the server.
            warp::serve(routes.with(cors).with(custom_log)).run(rest_ip).await
        })))
    }

    pub fn routes(&self) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        // GET /testnet3/solutions/rewards/{address}/{begin}/{end}
        let solutions_rewards = warp::get()
            .and(warp::path!("testnet3" / "solutions" / "rewards" / String / i64 / i64))
            .and(with(self.store.clone()))
            .and_then(Self::get_solutions_rewards);

        

        solutions_rewards
    }

   
}

impl<N: Network, S: Storage<N> + Send + Sync + 'static> Server<N, S> {
    pub async fn get_solutions_rewards(address: String, begin: i64, end: i64, store: Arc<Store<N, S>>) -> anyhow::Result<impl Reply, Rejection> {
        let result = store.get_solutions_by_time_range(&address, begin, end).map_or(Response::success(vec![]),|v| { 
            Response::success(v)
        });
        Ok(result.json())
    }
}

