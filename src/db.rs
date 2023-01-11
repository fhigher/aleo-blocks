use mysql::*;
use mysql::prelude::*;

pub struct MysqlClient {
    pool: Pool,
}

const TABLE_BLOCKS_NAME: &str = "blocks";
const TABLE_TRANSACTIONS_NAME: &str = "transactions";
const TABLE_SOLUTIONS_NAME: &str = "solutions";

impl<'a> MysqlClient {
    pub fn new(url: &'a str) -> Self {
        let pool = Pool::new(url).unwrap();
        Self {pool}
    }

    //pub fn db_latest_height(&self) 
}