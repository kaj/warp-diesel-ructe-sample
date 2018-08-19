use schema::users;

#[derive(Debug, Identifiable, Queryable)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub realname: String,
}
