use schema::users;
use std::io::{self, Write};
use templates::ToHtml;

#[derive(Debug, Identifiable, Queryable)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub realname: String,
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
