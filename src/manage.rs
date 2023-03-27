use reqwest::{Client, Response};
use log::{error, debug};
use crate::utils::{backoffset, from_reqwest_err};
use backoff::future::retry;
use std::collections::VecDeque;
use std::cell::RefCell;

#[derive(Clone, Debug)]
pub struct ApiManage {
    client: Client,
    apis: RefCell<VecDeque<String>>,
    _len: usize,
}

impl ApiManage {
    pub fn new(client: Client, apis: Vec<String>) -> Self {
        Self {client, _len: apis.len(), apis: RefCell::new(VecDeque::from(apis))}
    }

    pub async fn get(&self, url_path: &str) -> anyhow::Result<Response> {
        {
            debug!("{:?}", self.apis.borrow());
        }
        // loop {} 无限循环，直到有一个api可用
        // for _ in 0..self.len {  // 否则遍历完api，如果此时在batch阶段，遇错传递返回，进程就终止了。而在single阶段，调用方则不断continue重试
        loop {
            let apis_queue = self.apis.borrow();
            let api = apis_queue.front().unwrap();
            let url = format!("{}{}", *api, url_path);
            drop(apis_queue);

            match retry(backoffset(), || async {
                self.client.get(&url).send().await.map_err(from_reqwest_err) 
            }).await {
                Ok(response) => {
                    return Ok(response)
                },
                Err(error) => {
                    error!("reach backoffset max retry, failed to fetch {url}: {error}");
                    
                    let mut apis_queue = self.apis.borrow_mut();
                    let first = apis_queue.pop_front().unwrap();
                    apis_queue.push_back(first);

                    drop(apis_queue);
                    continue
                }
            }
        }

        // bail!("finally failed to fetch {url}")
    }
}