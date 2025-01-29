FROM public.ecr.aws/amazonlinux/amazonlinux:2023

RUN dnf makecache && \
    dnf install -y \
      gzip \
      nodejs \
      tar \
    && \
    dnf clean all && \
    rm -rf /var/cache/dnf

COPY --chmod=755 ./run.sh /
ENTRYPOINT ["./run.sh"]
