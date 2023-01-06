# Autarco Scaper

Autarco Scraper is a web service that provides a REST API layer over the My
Autarco site/API to get statistical data of your solar panels.

## Building & running

First, you need to provide your My Autarco credentials in the file
`Rocket.toml` by setting the username, password and site ID.
You can copy and modify `Rocket.toml.example` for this:

```toml
[default]
# ...

# Put your My Autarco credentials below and uncomment them
username = "foo@domain.tld"
password = "secret"
site_id = "abc123de"
```

You can also change this configuration to use a different address and/or port.
(Note that Rocket listens on `127.0.0.1:8000` by default for debug builds, i.e.
builds when you don't add `--release`.)

```toml
[default]
address = "0.0.0.0"
port = 8080

# ...
```

This will work independent of the type of build. For more about Rocket's
configuration, see: <https://rocket.rs/v0.5-rc/guide/configuration/>.

Finally, using Cargo, it is easy to build and run Autarco Scraper, just run:

```shell
$ cargo run --release
...
   Compiling autarco-scraper v0.1.1 (/path/to/autarco-scraper)
    Finished release [optimized] target(s) in 9m 26s
     Running `/path/to/autarco-scraper/target/release/autarco-scraper`
```

## API endpoint

The `/` API endpoint provides the current statistical data of your solar panels
once it has successfully logged into the My Autarco website using your
credentials. There is no path and no query parameters, just:

```http
GET /
```

### Response

A response uses the JSON format and typically looks like this:

```json
{"current_w":23,"total_kwh":6159,"last_updated":1661194620}
```

This contains the current production power (`current_w`) in Watt,
the total of produced energy since installation (`total_kwh`) in kilowatt-hour
and the (UNIX) timestamp that indicates when the information was last updated.

## License

Autarco Scraper is licensed under the MIT license (see the `LICENSE` file or
<http://opensource.org/licenses/MIT>).
