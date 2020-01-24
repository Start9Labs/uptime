FROM alpine

EXPOSE 59002

ADD ./target/armv7-unknown-linux-musleabihf/release/uptime /usr/local/bin/uptime
ADD ./docker_entrypoint.sh /usr/local/bin/docker_entrypoint.sh
RUN chmod a+x /usr/local/bin/docker_entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker_entrypoint.sh"]
