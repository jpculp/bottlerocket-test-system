FROM public.ecr.aws/amazonlinux/amazonlinux:2023

RUN dnf makecache && \
    dnf install -y \
      gzip \
      go \
      tar \
    && \
    dnf clean all && \
    rm -rf /var/cache/dnf

COPY --chmod=755 ./run.sh /
ENTRYPOINT ["./run.sh"]
