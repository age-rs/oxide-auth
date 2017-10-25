#[cfg(feature = "iron-backend")]
mod main {
    extern crate oauth2_server;
    extern crate iron;
    extern crate router;
    extern crate url;
    extern crate reqwest;
    extern crate urlencoded;
    use self::iron::prelude::*;
    use self::oauth2_server::iron::prelude::*;
    use self::urlencoded::UrlEncodedQuery;
    use std::collections::HashMap;
    use std::thread;

    pub fn exec() {
        let ohandler = IronGranter::new(
            ClientMap::new(),
            Storage::new(RandomGenerator::new(16)),
            TokenMap::new(RandomGenerator::new(32)));
        ohandler.registrar().unwrap().register_client("myself", url::Url::parse("http://localhost:8021/endpoint").unwrap());

        let mut router = router::Router::new();
        router.get("/authorize", ohandler.authorize(handle_get), "authorize");
        router.post("/authorize", ohandler.authorize(Box::new(handle_post) as Box<iron::Handler>), "authorize");
        router.post("/token", ohandler.token(), "token");

        let join = thread::spawn(|| iron::Iron::new(router).http("localhost:8020").unwrap());
        let client = thread::spawn(|| iron::Iron::new(dummy_client).http("localhost:8021").unwrap());
        open_in_browser();
        join.join().expect("Failed to run");
        client.join().expect("Failed to run client");

        /// A simple implementation of the first part of an authentication handler. This will
        /// display a page to the user asking for his permission to proceed. The submitted form
        /// will then trigger the other authorization handler which actually completes the flow.
        fn handle_get(_: &mut Request, auth: AuthenticationRequest) -> Result<(Authentication, Response), OAuthError> {
            let (client_id, scope) = (auth.client_id, auth.scope);
            let text = format!(
                "<html>{} is requesting permission for {}
                <form action=\"authorize?response_type=code&client_id={}\" method=\"post\">
                    <input type=\"submit\" value=\"Accept\">
                </form>
                <form action=\"authorize?response_type=code&client_id={}&deny=1\" method=\"post\">
                    <input type=\"submit\" value=\"Deny\">
                </form>
                </html>", client_id, scope, client_id, client_id);
            let response = Response::with((iron::status::Ok, iron::modifiers::Header(iron::headers::ContentType::html()), text));
            Ok((Authentication::InProgress, response))
        }

        /// This shows the second style of authentication handler, a iron::Handler compatible form.
        /// Allows composition with other libraries or frameworks.
        fn handle_post(req: &mut Request) -> IronResult<Response> {
            // No real user authentication is done here, in production you could use session keys
            if req.get::<UrlEncodedQuery>().unwrap_or(HashMap::new()).contains_key("deny") {
                req.extensions.insert::<Authentication>(Authentication::Failed);
            } else {
                req.extensions.insert::<Authentication>(Authentication::Authenticated("dummy user".to_string()));
            }
            Ok(Response::with(iron::status::Ok))
        }
    }

    fn open_in_browser() {
        let target_addres = "localhost:8020/authorize?response_type=code&client_id=myself";
        use std::io::{Error, ErrorKind};
        use std::process::Command;
        let can_open = if cfg!(target_os = "linux") {
            Ok("x-www-browser")
        } else {
            Err(Error::new(ErrorKind::Other, "Open not supported"))
        };
        can_open.and_then(|cmd| Command::new(cmd).arg(target_addres).status())
            .and_then(|status| if status.success() { Ok(()) } else { Err(Error::new(ErrorKind::Other, "Non zero status")) })
            .unwrap_or_else(|_| println!("Please navigate to {}", target_addres));
    }

    /// Rough client function mirroring core functionality of an oauth client. This is not actually
    /// needed in your implementation but merely exists to provide an interactive example.
    fn dummy_client(req: &mut iron::Request) -> iron::IronResult<iron::Response> {
        use std::io::Read;
        let code = match req.url.as_ref().query_pairs().collect::<QueryMap>().get("code") {
            None => return Ok(iron::Response::with((iron::status::BadRequest, "Missing code"))),
            Some(v) => v.clone()
        };

        let client = reqwest::Client::new();
        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code");
        params.insert("client_id", "myself");
        params.insert("code", &code);
        params.insert("redirect_url", "http://localhost:8021/endpoint");
        let constructed_req = client
            .post("http://localhost:8020/token")
            .form(&params).build().unwrap();
        let mut token_req = match client.execute(constructed_req) {
            Err(_) => return Ok(iron::Response::with((iron::status::InternalServerError, "Error retrieving token from server"))),
            Ok(v) => v
        };
        let mut token = String::new();
        token_req.read_to_string(&mut token).unwrap();
        Ok(iron::Response::with((iron::status::Ok, format!("Received token: {}", token))))
    }
}

#[cfg(not(feature = "iron-backend"))]
mod main { pub fn exec() { } }

fn main() {
    main::exec();
}
