# Builds the SoulSync run server. Build from the repository root:
#   docker build -f deploy/Dockerfile -t soulsync-server .
FROM rust:1-slim AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p soulsync-server

FROM debian:stable-slim
COPY --from=build /src/target/release/soulsync-server /usr/local/bin/soulsync-server
# Runs are kept here across restarts; mount a volume on it.
RUN mkdir /data && chown nobody /data
ENV PORT=3000 SOULSYNC_DATA=/data
EXPOSE 3000
USER nobody
CMD ["soulsync-server"]
