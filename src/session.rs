use diesel::insert_into;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::result::Error;
use models::User;
use rand::distributions::Alphanumeric;
use rand::thread_rng;
use rand::Rng;

type PooledPg = PooledConnection<ConnectionManager<PgConnection>>;
type PgPool = Pool<ConnectionManager<PgConnection>>;

/// A Session object is sent to most handler methods.
///
/// The content of the session object is application specific.
/// My session contains a session pool for the database and an
/// optional user (if logged in).
/// It may also contain pools to other backend servers (e.g. memcache,
/// redis, or application specific services) and/or other temporary
/// user data (e.g. a shopping cart in a web shop).
pub struct Session {
    db: PooledPg,
    user: Option<User>,
}

impl Session {
    pub fn from_key(db: PooledPg, sessionkey: Option<&str>) -> Self {
        use schema::sessions::dsl as s;
        use schema::users::dsl as u;
        let user = sessionkey.and_then(|sessionkey| {
            u::users
                .select((u::id, u::username, u::realname))
                .inner_join(s::sessions)
                .filter(s::cookie.eq(&sessionkey))
                .first::<User>(&db)
                .ok()
        });
        info!("Got: {:?}", user);
        Session { db, user }
    }
    pub fn create(&self, userid: i32) -> Result<String, Error> {
        let secret_cookie = random_key(48);
        use schema::sessions::dsl::*;
        insert_into(sessions)
            .values((user_id.eq(userid), cookie.eq(&secret_cookie)))
            .execute(self.db())?;
        Ok(secret_cookie)
    }
    pub fn user(&self) -> Option<&User> {
        self.user.as_ref()
    }
    pub fn db(&self) -> &PgConnection {
        &self.db
    }
}

fn random_key(len: usize) -> String {
    let mut rng = thread_rng();
    rng.sample_iter(&Alphanumeric).take(len).collect()
}

// TODO Not public
pub fn pg_pool(database_url: &str) -> PgPool {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::new(manager).expect("Postgres connection pool could not be created")
}
