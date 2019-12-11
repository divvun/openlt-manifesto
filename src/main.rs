use handlebars::Handlebars;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tide::prelude::ResultExt;
use tide::IntoResponse;

type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
type Connection = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

#[derive(Debug, Serialize)]
struct Signatory {
    name: String,
    title: Option<String>,
    organisation: Option<String>,
    url: Option<String>,
    comment: Option<String>,
}

fn get_signatories(conn: &Connection) -> Result<Vec<Signatory>, rusqlite::Error> {
    let stmt = "SELECT name, title, organisation, url, comment FROM signatories WHERE type = 'sig' ORDER BY random();";

    let mut prep_stmt = conn.prepare(stmt).unwrap();
    prep_stmt
        .query_map(rusqlite::NO_PARAMS, |row| {
            Ok(Signatory {
                name: row.get(0)?,
                title: row.get(1)?,
                organisation: row.get(2)?,
                url: row.get(3)?,
                comment: row.get(4)?,
            })
        })
        .and_then(|mapped_rows| Ok(mapped_rows.map(|row| row.unwrap()).collect::<Vec<_>>()))
}

fn get_quotes(conn: &Connection) -> Result<Vec<Signatory>, rusqlite::Error> {
    let stmt = "SELECT name, title, organisation, url, comment FROM signatories WHERE type = 'sig' AND comment <> '' ORDER BY random() LIMIT 3;";

    let mut prep_stmt = conn.prepare(stmt).unwrap();
    prep_stmt
        .query_map(rusqlite::NO_PARAMS, |row| {
            Ok(Signatory {
                name: row.get(0)?,
                title: row.get(1)?,
                organisation: row.get(2)?,
                url: row.get(3)?,
                comment: row.get(4)?,
            })
        })
        .and_then(|mapped_rows| Ok(mapped_rows.map(|row| row.unwrap()).collect::<Vec<_>>()))
}

#[derive(Debug, Serialize)]
struct IndexResponse {
    signatories: Vec<Signatory>,
    quotes: Vec<Signatory>,
}

struct State {
    pool: Pool,
    handlebars: Handlebars,
}

async fn index_inner(req: tide::Request<State>) -> Result<tide::Response, tide::Error> {
    let pool = req.state().pool.clone();
    let db = pool.get().with_err_status(500)?;

    let signatories = get_signatories(&db).with_err_status(500)?;
    let quotes = get_quotes(&db).with_err_status(500)?;

    let hb = &req.state().handlebars;

    let body = hb
        .render(
            "index",
            &IndexResponse {
                signatories,
                quotes,
            },
        )
        .with_err_status(500)?;

    Ok(tide::Response::new(200)
        .body_string(body)
        .set_mime(mime::TEXT_HTML_UTF_8))
}

#[derive(Debug, Deserialize)]
struct SignatureForm {
    name: Option<String>,
    title: Option<String>,
    email: Option<String>,
    organisation: Option<String>,
    url: Option<String>,
    comments: Option<String>,
    mailing_list_opt_in: Option<String>,
}

fn assert_not_blank(mut s: Option<String>) -> Result<String, tide::Error> {
    match s.take() {
        Some(v) => {
            let v = v.trim();
            if v == "" {
                return Err(tide::Response::new(400).into());
            }
            Ok(v.to_string())
        }
        None => return Err(tide::Response::new(400).into()),
    }
}

async fn index_post_inner(mut req: tide::Request<State>) -> Result<tide::Response, tide::Error> {
    let body: SignatureForm = req.body_form().await.with_err_status(400)?;
    let pool = req.state().pool.clone();
    let db = pool.get().with_err_status(500)?;

    let name = assert_not_blank(body.name)?;
    let email = assert_not_blank(body.email)?;
    let url = match assert_not_blank(body.url) {
        Ok(v) => {
            let url = match url::Url::parse(&v) {
                Ok(v) => v,
                Err(_) => match url::Url::parse(&format!("http://{}", v)) {
                    Ok(v) => v,
                    Err(_) => return Err(tide::Error::from(tide::Response::new(400))),
                },
            };
            Some(url.to_string())
        }
        Err(_) => None,
    };
    let title = assert_not_blank(body.title).ok();
    let organisation = assert_not_blank(body.organisation).ok();
    let comments = assert_not_blank(body.comments).ok();
    let mailing_list_opt_in: i32 = match body.mailing_list_opt_in {
        Some(v) => {
            if v == "on" {
                1
            } else {
                0
            }
        }
        _ => 0,
    };
    let created_on = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::new(0, 0))
        .as_secs() as i64;

    db.execute(
        r"INSERT INTO signatories (
        name, title, email, organisation, url, comment, mailing_list_opt_in, created_on
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            name,
            title,
            email,
            organisation,
            url,
            comments,
            mailing_list_opt_in,
            created_on
        ],
    )
    .map_err(|_| tide::Error::from(tide::Response::new(400)))?;

    Ok(tide::Response::new(303).set_header("Location", "/success"))
}

async fn success(req: tide::Request<State>) -> tide::Response {
    async move {
        let hb = &req.state().handlebars;
        let body = hb.render("success", &json!({})).with_err_status(500)?;
        Ok(tide::Response::new(200)
            .body_string(body)
            .set_mime(mime::TEXT_HTML_UTF_8))
    }
    .await
    .unwrap_or_else(|e: tide::Error| e.into_response())
}

async fn privacy(req: tide::Request<State>) -> tide::Response {
    async move {
        let hb = &req.state().handlebars;
        let body = hb.render("privacy", &json!({})).with_err_status(500)?;
        Ok(tide::Response::new(200)
            .body_string(body)
            .set_mime(mime::TEXT_HTML_UTF_8))
    }
    .await
    .unwrap_or_else(|e: tide::Error| e.into_response())
}

async fn index(req: tide::Request<State>) -> tide::Response {
    // TODO show actual errors
    index_inner(req).await.unwrap_or_else(|e| e.into_response())
}

async fn index_post(req: tide::Request<State>) -> tide::Response {
    index_post_inner(req)
        .await
        .unwrap_or_else(|e| e.into_response())
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    let manager = SqliteConnectionManager::file("signatories.db");
    let pool = r2d2::Pool::new(manager).unwrap();
    let mut handlebars = Handlebars::new();
    handlebars
        .register_templates_directory(".html", "./static/templates")
        .unwrap();

    let mut app = tide::with_state(State { pool, handlebars });
    app.at("/").get(index);
    app.at("/privacy").get(privacy);
    app.at("/success").get(success);
    app.at("/submit").post(index_post);
    app.listen("127.0.0.1:8080").await?;
    Ok(())
}
