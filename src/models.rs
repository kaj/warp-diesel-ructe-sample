use crate::templates::ToHtml;
use bcrypt;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use log::error;
use std::io::{self, Write};

#[derive(Debug, Queryable)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub realname: String,
}

impl User {
    pub fn authenticate(
        db: &PgConnection,
        user: &str,
        pass: &str,
    ) -> Option<Self> {
        use crate::schema::users::dsl::*;
        let (user, hash) = match users
            .filter(username.eq(user))
            .select(((id, username, realname), password))
            .first::<(User, String)>(db)
        {
            Ok((user, hash)) => (user, hash),
            Err(e) => {
                error!("Failed to load hash for {:?}: {:?}", user, e);
                return None;
            }
        };

        match bcrypt::verify(&pass, &hash) {
            Ok(true) => Some(user),
            Ok(false) => None,
            Err(e) => {
                error!("Verify failed for {:?}: {:?}", user, e);
                None
            }
        }
    }
}

// Implementing ToHtml for a type makes it possible to use that type
// directly in templates.
impl ToHtml for User {
    fn to_html(&self, out: &mut Write) -> io::Result<()> {
        out.write_all(b"<span data-user=\"")?;
        self.username.to_html(out)?;
        out.write_all(b"\">")?;
        self.realname.to_html(out)?;
        out.write_all(b"</span>")?;
        Ok(())
    }
}
