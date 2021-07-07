
FROM debian:10-slim AS build

RUN mkdir -p /opt/app
WORKDIR /opt/app

COPY water-levels ./water-levels
RUN chmod 755 ./water-levels

RUN mkdir -p /opt/app/static
COPY frontend/public/bundle.js ./static
COPY frontend/index.html ./static
COPY frontend/favicon/* ./static

FROM gcr.io/distroless/cc-debian10

COPY --from=build /opt/app/ /opt/app/

WORKDIR /opt/app/

ENV PORT=9002
EXPOSE ${PORT}

ENTRYPOINT ["/opt/app/water-levels"]
