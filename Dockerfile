# Using multistage build:
#   https://docs.docker.com/develop/develop-images/multistage-build/
#   https://whitfin.io/speeding-up-rust-docker-builds/

##########################  BUILD IMAGE  ##########################
# Rust build image to build Autarco Scraper's statically compiled binary
FROM rust:1.45 as builder

# Build the dependencies first
RUN USER=root cargo new --bin autarco-scraper
WORKDIR ./autarco-scraper
COPY ./Cargo.* ./
RUN cargo build --release
RUN rm src/*.rs

# Add the real project files from current folder
ADD . ./

# Build the actual binary from the copied local files
RUN rm ./target/release/deps/autarco_scraper*
RUN cargo build --release

########################## RUNTIME IMAGE ##########################
# Create new stage with a minimal image for the actual runtime image/container
FROM debian:buster-slim

# Install cURL, Firefox and the Gecko Driver
RUN apt-get update && \
      apt-get upgrade -y && \
      apt-get install -y --no-install-recommends ca-certificates curl firefox-esr jq && \
      rm -rf /var/lib/apt/lists/*
RUN export VERSION=$(curl -sL https://api.github.com/repos/mozilla/geckodriver/releases/latest | jq -r .tag_name); \
      curl -vsL https://api.github.com/repos/mozilla/geckodriver/releases/latest | jq -r .tag_name; \
      curl -sL "https://github.com/mozilla/geckodriver/releases/download/$VERSION/geckodriver-$VERSION-linux64.tar.gz" | \
      tar -xz -C /usr/local/bin && \
      chmod +x /usr/local/bin/geckodriver

# Copy the binary from the "builder" stage to the current stage
RUN adduser --system --disabled-login --home /autarco-scraper --gecos "" --shell /bin/bash autarco-scraper
COPY --from=builder /autarco-scraper/target/release/autarco-scraper /autarco-scraper

# Standard port on which Rocket launches
EXPOSE 8000

# Set user to www-data
USER autarco-scraper

# Set container home directory
WORKDIR /autarco-scraper

# Run Autarco Scraper
ENTRYPOINT [ "/autarco-scraper/autarco-scraper" ]
