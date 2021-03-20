use crate::models::User;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use log::{debug, error};
use rand::distributions::Alphanumeric;
use rand::thread_rng;
use rand::Rng;
use warp::filters::{cookie, BoxedFilter};
use warp::{self, Filter};

type PooledPg = PooledConnection<ConnectionManager<PgConnection>>;
type PgPool = Pool<ConnectionManager<PgConnection>>;

#[derive(Debug)]
pub struct NoDbReady;
impl warp::reject::Reject for NoDbReady {}

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
    id: Option<i32>,
    user: Option<User>,
}

impl Session {
    /// Attempt to authenticate a user for this session.
    ///
    /// If the username and password is valid, create and return a session key.
    /// If authentication fails, simply return None.
    pub fn authenticate(
        &mut self,
        username: &str,
        password: &str,
    ) -> Option<String> {
        if let Some(user) = User::authenticate(self.db(), username, password)
        {
            debug!("User {:?} authenticated", user);

            let secret = random_key(48);
            use crate::schema::sessions::dsl::*;
            let result = diesel::insert_into(sessions)
                .values((user_id.eq(user.id), cookie.eq(&secret)))
                .returning(id)
                .get_results(self.db());
            if let Ok([a]) = result.as_ref().map(|v| &**v) {
                self.id = Some(*a);
                self.user = Some(user);
                return Some(secret);
            } else {
                error!(
                    "Failed to create session for {}: {:?}",
                    user.username, result,
                );
            }
        }
        None
    }

    /// Get a Session from a database pool and a session key.
    ///
    /// The session key is checked against the database, and the
    /// matching session is loaded.
    /// The database pool handle is included in the session regardless
    /// of if the session key is a valid session or not.
    pub fn from_key(db: PooledPg, sessionkey: Option<&str>) -> Self {
        use crate::schema::sessions::dsl as s;
        use crate::schema::users::dsl as u;
        let (id, user) = sessionkey
            .and_then(|sessionkey| {
                u::users
                    .inner_join(s::sessions)
                    .select((s::id, (u::id, u::username, u::realname)))
                    .filter(s::cookie.eq(&sessionkey))
                    .first::<(i32, User)>(&db)
                    .ok()
            })
            .map(|(i, u)| (Some(i), Some(u)))
            .unwrap_or((None, None));

        debug!("Got: #{:?} {:?}", id, user);
        Session { db, id, user }
    }

    /// Clear the part of this session that is session-specific.
    ///
    /// In effect, the database pool will remain, but the user will be
    /// cleared, and the data in the sessions table for this session
    /// will be deleted.
    pub fn clear(&mut self) {
        use crate::schema::sessions::dsl as s;
        if let Some(session_id) = self.id {
            diesel::delete(s::sessions.filter(s::id.eq(session_id)))
                .execute(self.db())
                .map_err(|e| {
                    error!(
                        "Failed to delete session {}: {:?}",
                        session_id, e
                    );
                })
                .ok();
        }
        self.id = None;
        self.user = None;
    }

    pub fn user(&self) -> Option<&User> {
        self.user.as_ref()
    }
    pub fn db(&self) -> &PgConnection {
        &self.db
    }
}

fn random_key(len: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .map(char::from)
        .take(len)
        .collect()
}

pub fn create_session_filter(db_url: &str) -> BoxedFilter<(Session,)> {
    let pool = pg_pool(db_url);
    warp::any()
        .and(cookie::optional("EXAUTH"))
        .and_then(move |key: Option<String>| {
            let pool = pool.clone();
            async move {
                let key = key.as_ref().map(|s| &**s);
                match pool.get() {
                    Ok(conn) => Ok(Session::from_key(conn, key)),
                    Err(e) => {
                        error!("Failed to get a db connection: {}", e);
                        Err(warp::reject::custom(NoDbReady))
                    }
                }
            }
        })
        .boxed()
}

fn pg_pool(database_url: &str) -> PgPool {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::new(manager).expect("Postgres connection pool could not be created")
}
