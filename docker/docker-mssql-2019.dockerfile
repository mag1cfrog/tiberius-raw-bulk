FROM mcr.microsoft.com/mssql/server:2019-latest

USER root
COPY certs/server.* /certs/
COPY certs/customCA.* /certs/
RUN chown mssql /certs/server.* /certs/customCA.* && chmod 440 /certs/server.* /certs/customCA.*
COPY docker-mssql.conf /var/opt/mssql/mssql.conf
RUN chown mssql /var/opt/mssql/mssql.conf
USER mssql
