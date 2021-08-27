# Using multistage build:
#   https://docs.docker.com/develop/develop-images/multistage-build/
#   https://whitfin.io/speeding-up-rust-docker-builds/

##########################  BUILD IMAGE  ##########################
# Rust build image to build Autarco Scraper's statically compiled binary
FROM docker.io/rust:1.54 as builder

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
FROM docker.io/debian:bullseye-slim

# Install CA certificates
RUN apt-get update && \
      apt-get upgrade -y && \
      apt-get install -y --no-install-recommends ca-certificates && \
      rm -rf /var/lib/apt/lists/*

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
