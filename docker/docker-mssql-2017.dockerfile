FROM mcr.microsoft.com/mssql/server:2017-latest

USER root
COPY certs/server.* /certs/
COPY certs/customCA.* /certs/
RUN chmod 440 /certs/server.* /certs/customCA.*
COPY docker-mssql.conf /var/opt/mssql/mssql.conf
