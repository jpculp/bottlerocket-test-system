# syntax=docker/dockerfile:1.4.3
# This Dockfile contains separate targets for each testsys agent
ARG BUILDER_IMAGE
FROM "${BUILDER_IMAGE}" as base

USER root
COPY ./hashes /hashes
COPY ./clarify.toml /clarify.toml
COPY ./eksctl.clarify.toml /eksctl.clarify.toml
COPY ./sonobuoy.clarify.toml /sonobuoy.clarify.toml

# =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^=
# Shared build stage used to build Go binaries.
FROM base as build-go

ARG GOARCH
ARG GOOS=linux
ARG GOPROXY

USER builder
ENV GOPROXY="${GOPROXY}"

# =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^=
FROM build-go as eksctl-build

USER root
RUN mkdir -p /usr/share/licenses/eksctl && \
    chown -R builder:builder /usr/share/licenses/eksctl

ARG EKSCTL_VERSION=0.191.0
ARG EKSCTL_SOURCE_URL="https://github.com/eksctl-io/eksctl/archive/refs/tags/v${EKSCTL_VERSION}.tar.gz"

ARG EKSCTL_BINARY_URL="https://github.com/eksctl-io/eksctl/releases/download/v${EKSCTL_VERSION}/eksctl_Linux_${GOARCH}.tar.gz"

USER builder
WORKDIR /home/builder/
RUN mkdir eksctl && \
    curl -L "${EKSCTL_SOURCE_URL}" \
      -o "eksctl_${EKSCTL_VERSION}.tar.gz" && \
    grep "eksctl_${EKSCTL_VERSION}.tar.gz" \
      /hashes/eksctl | sha512sum --check - && \
    tar -xf "eksctl_${EKSCTL_VERSION}.tar.gz" \
      --strip-components 1 -C eksctl && \
    rm "eksctl_${EKSCTL_VERSION}.tar.gz"

WORKDIR /home/builder/eksctl/
RUN go mod vendor
RUN cp -p LICENSE /usr/share/licenses/eksctl && \
    /usr/libexec/tools/bottlerocket-license-scan \
      --clarify /eksctl.clarify.toml \
      --spdx-data /usr/libexec/tools/spdx-data \
      --out-dir /usr/share/licenses/eksctl/vendor \
      go-vendor ./vendor
RUN curl -L "${EKSCTL_BINARY_URL}" \
      -o "eksctl_${EKSCTL_VERSION}_${GOOS}_${GOARCH}.tar.gz" && \
    grep "eksctl_${EKSCTL_VERSION}_${GOOS}_${GOARCH}.tar.gz" \
      /hashes/eksctl | sha512sum --check - && \
    tar -xf "eksctl_${EKSCTL_VERSION}_${GOOS}_${GOARCH}.tar.gz" && \
    rm "eksctl_${EKSCTL_VERSION}_${GOOS}_${GOARCH}.tar.gz"

# =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^=
FROM build-go as kubernetes-build

USER root
RUN dnf -y install yq
RUN mkdir -p /usr/share/licenses/kubernetes && \
    chown -R builder:builder /usr/share/licenses/kubernetes

ARG K8S_MINOR_VERSION=1-27
ARG EKSD_PATCH_VERSION=35

ARG K8S_SOURCE_URL="https://github.com/aws/eks-distro/archive/refs/tags/v${K8S_MINOR_VERSION}-eks-${EKSD_PATCH_VERSION}.tar.gz"
ARG EKSD_RELEASE_MANIFEST="https://distro.eks.amazonaws.com/kubernetes-${K8S_MINOR_VERSION}/kubernetes-${K8S_MINOR_VERSION}-eks-${EKSD_PATCH_VERSION}.yaml"

USER builder
WORKDIR /home/builder/
RUN mkdir kubernetes && \
    curl -L "${K8S_SOURCE_URL}" -o "kubernetes_${K8S_MINOR_VERSION}-${EKSD_PATCH_VERSION}.tar.gz" && \
    grep "kubernetes_${K8S_MINOR_VERSION}-${EKSD_PATCH_VERSION}.tar.gz" \
      /hashes/kubernetes | sha512sum --check - && \
    tar -xf "kubernetes_${K8S_MINOR_VERSION}-${EKSD_PATCH_VERSION}.tar.gz" \
      --strip-components 1 -C kubernetes && \
    rm "kubernetes_${K8S_MINOR_VERSION}-${EKSD_PATCH_VERSION}.tar.gz"

WORKDIR /home/builder/kubernetes/

# We don't need to run `go mod vendor` to generate the vendored code:
# upstream kubernetes already vendors all its dependencies
RUN cp -p LICENSE /usr/share/licenses/kubernetes && \
    go mod vendor && \
    /usr/libexec/tools/bottlerocket-license-scan \
      --clarify /clarify.toml \
      --spdx-data /usr/libexec/tools/spdx-data \
      --out-dir /usr/share/licenses/kubernetes/vendor \
      go-vendor ./vendor

RUN curl -L "${EKSD_RELEASE_MANIFEST}" -o "kubernetes-${K8S_MINOR_VERSION}-eks-${EKSD_PATCH_VERSION}.yaml" && \
  grep "kubernetes-${K8S_MINOR_VERSION}-eks-${EKSD_PATCH_VERSION}.yaml" \
    /hashes/kubernetes | sha512sum --check -

RUN yq ".status.components.[] | \
  select(.name ==  \"kubernetes\").assets.[] | \
  select(.name ==  \"bin/${GOOS}/${GOARCH}/kubeadm\") \
  .archive.uri" "kubernetes-${K8S_MINOR_VERSION}-eks-${EKSD_PATCH_VERSION}.yaml" | \
  xargs curl -L \
      -o "kubeadm_${K8S_MINOR_VERSION}-${EKSD_PATCH_VERSION}_${GOOS}_${GOARCH}" && \
  EXPECTED_CHECKSUM=$(yq ".status.components.[] | \
    select(.name == \"kubernetes\").assets.[] | \
    select(.name == \"bin/${GOOS}/${GOARCH}/kubeadm\") \
    .archive.sha512" "kubernetes-${K8S_MINOR_VERSION}-eks-${EKSD_PATCH_VERSION}.yaml") && \
  sha512sum "kubeadm_${K8S_MINOR_VERSION}-${EKSD_PATCH_VERSION}_${GOOS}_${GOARCH}" | \
    awk "\$1!=\"${EXPECTED_CHECKSUM}\"{print \"checksum mismatch\"; exit 1}" && \
  install -m 0755 "kubeadm_${K8S_MINOR_VERSION}-${EKSD_PATCH_VERSION}_${GOOS}_${GOARCH}" ./kubeadm

RUN yq ".status.components.[] | \
    select(.name == \"kubernetes\").assets.[] | \
    select(.name == \"bin/${GOOS}/${GOARCH}/kubectl\") \
    .archive.uri" "kubernetes-${K8S_MINOR_VERSION}-eks-${EKSD_PATCH_VERSION}.yaml" | \
  xargs curl -L \
      -o "kubectl_${K8S_MINOR_VERSION}-${EKSD_PATCH_VERSION}_${GOOS}_${GOARCH}" && \
  EXPECTED_CHECKSUM=$(yq ".status.components.[] | \
    select(.name == \"kubernetes\").assets.[] | \
    select(.name == \"bin/${GOOS}/${GOARCH}/kubectl\") \
    .archive.sha512" "kubernetes-${K8S_MINOR_VERSION}-eks-${EKSD_PATCH_VERSION}.yaml") && \
  sha512sum "kubectl_${K8S_MINOR_VERSION}-${EKSD_PATCH_VERSION}_${GOOS}_${GOARCH}" | \
    awk "\$1!=\"${EXPECTED_CHECKSUM}\"{print \"checksum mismatch\"; exit 1}" && \
  install -m 0755 "kubectl_${K8S_MINOR_VERSION}-${EKSD_PATCH_VERSION}_${GOOS}_${GOARCH}" ./kubectl

# =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^=
FROM build-go as sonobuoy-build

USER root
RUN mkdir -p /usr/share/licenses/sonobuoy && \
    chown -R builder:builder /usr/share/licenses/sonobuoy

ARG SONOBUOY_VERSION=0.57.1
ARG SONOBUOY_SOURCE_URL="https://github.com/vmware-tanzu/sonobuoy/archive/refs/tags/v${SONOBUOY_VERSION}.tar.gz"

ARG SONOBUOY_BINARY_URL="https://github.com/vmware-tanzu/sonobuoy/releases/download/v${SONOBUOY_VERSION}/sonobuoy_${SONOBUOY_VERSION}_linux_${GOARCH}.tar.gz"

USER builder
WORKDIR /home/builder/
RUN mkdir sonobuoy && \
    curl -L "${SONOBUOY_SOURCE_URL}" \
      -o "sonobuoy_${SONOBUOY_VERSION}.tar.gz" && \
    grep "sonobuoy_${SONOBUOY_VERSION}.tar.gz" \
      /hashes/sonobuoy | sha512sum --check - && \
    tar -xf "sonobuoy_${SONOBUOY_VERSION}.tar.gz" \
      --strip-components 1 -C sonobuoy && \
    rm "sonobuoy_${SONOBUOY_VERSION}.tar.gz"

WORKDIR /home/builder/sonobuoy/
RUN go mod vendor
RUN cp -p LICENSE /usr/share/licenses/sonobuoy && \
    /usr/libexec/tools/bottlerocket-license-scan \
      --clarify /sonobuoy.clarify.toml \
      --spdx-data /usr/libexec/tools/spdx-data \
      --out-dir /usr/share/licenses/sonobuoy/vendor \
      go-vendor ./vendor
RUN curl -OL "${SONOBUOY_BINARY_URL}" && \
    grep "sonobuoy_${SONOBUOY_VERSION}_${GOOS}_${GOARCH}.tar.gz" \
      /hashes/sonobuoy | sha512sum --check - && \
    tar -xf "sonobuoy_${SONOBUOY_VERSION}_${GOOS}_${GOARCH}.tar.gz" && \
    chmod 0755 sonobuoy && \
    rm "sonobuoy_${SONOBUOY_VERSION}_${GOOS}_${GOARCH}.tar.gz"

# # =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^=
FROM build-go as helm-build

USER root
RUN mkdir -p /usr/share/licenses/helm && \
    chown -R builder:builder /usr/share/licenses/helm

ARG HELM_VERSION=3.15.3
ARG HELM_SOURCE_URL="https://github.com/helm/helm/archive/refs/tags/v${HELM_VERSION}.tar.gz"

# ARG HELM_BINARY_URL="https://github.com/helm/helm/releases/download/v${HELM_VERSION}/helm-v${HELM_VERSION}-${GOOS}-${GOARCH}.tar.gz.asc"
ARG HELM_BINARY_URL="https://get.helm.sh/helm-v${HELM_VERSION}-${GOOS}-${GOARCH}.tar.gz"


USER builder
WORKDIR /home/builder/
RUN mkdir helm && \
    curl -L "${HELM_SOURCE_URL}" \
      -o "helm_${HELM_VERSION}.tar.gz" && \
    grep "helm_${HELM_VERSION}.tar.gz" \
      /hashes/helm | sha512sum --check - && \
    tar -xf "helm_${HELM_VERSION}.tar.gz" \
      --strip-components 1 -C helm && \
    rm "helm_${HELM_VERSION}.tar.gz"

WORKDIR /home/builder/helm/
RUN go mod vendor
RUN cp -p LICENSE /usr/share/licenses/helm && \
    /usr/libexec/tools/bottlerocket-license-scan \
      --clarify /clarify.toml \
      --spdx-data /usr/libexec/tools/spdx-data \
      --out-dir /usr/share/licenses/helm/vendor \
      go-vendor ./vendor
RUN curl -L "${HELM_BINARY_URL}" \
      -o "helm_${HELM_VERSION}_${GOOS}_${GOARCH}.tar.gz" && \
    grep "helm_${HELM_VERSION}_${GOOS}_${GOARCH}.tar.gz" \
      /hashes/helm | sha512sum --check - && \
    tar -xf "helm_${HELM_VERSION}_${GOOS}_${GOARCH}.tar.gz" && \
    mv linux-${GOARCH}/helm helm && \
    chmod 0755 helm && \
    rm "helm_${HELM_VERSION}_${GOOS}_${GOARCH}.tar.gz"

# =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^=
FROM build-go as aws-iam-authenticator-build

USER root
RUN mkdir -p /usr/share/licenses/aws-iam-authenticator && \
    chown -R builder:builder /usr/share/licenses/aws-iam-authenticator

ARG AWS_IAM_AUTHENTICATOR_VERSION=0.6.21
ARG AWS_IAM_AUTHENTICATOR_SHA512_SUM=5548748efa330fc89256fda0e723e75f83040f55301f692166588d32c883a69d8f955a86f38c39558a6ccd65de6b622a59b7768486cc9ba6f1d78b2f9d183eed
ARG AWS_IAM_AUTHENTICATOR_SOURCE_URL="https://cache.bottlerocket.aws/aws-iam-authenticator-${AWS_IAM_AUTHENTICATOR_VERSION}.tar.gz/${AWS_IAM_AUTHENTICATOR_SHA512_SUM}/aws-iam-authenticator-${AWS_IAM_AUTHENTICATOR_VERSION}.tar.gz"

USER builder
WORKDIR /home/builder/
RUN mkdir aws-iam-authenticator && \
    curl -L "${AWS_IAM_AUTHENTICATOR_SOURCE_URL}" \
      -o "aws-iam-authenticator_${AWS_IAM_AUTHENTICATOR_VERSION}.tar.gz" && \
    grep "aws-iam-authenticator_${AWS_IAM_AUTHENTICATOR_VERSION}.tar.gz" \
      /hashes/aws-iam-authenticator | sha512sum --check - && \
    tar -xf "aws-iam-authenticator_${AWS_IAM_AUTHENTICATOR_VERSION}.tar.gz" \
      --strip-components 1 -C aws-iam-authenticator && \
    rm "aws-iam-authenticator_${AWS_IAM_AUTHENTICATOR_VERSION}.tar.gz"

WORKDIR /home/builder/aws-iam-authenticator/
RUN go mod vendor
RUN CGO_ENABLED=0 go build -mod=vendor ./cmd/aws-iam-authenticator

# =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^= =^..^=
# Package the binaries for use by other container image builds.
FROM scratch
# aws-iam-authenticator
COPY --from=aws-iam-authenticator-build \
     /home/builder/aws-iam-authenticator/aws-iam-authenticator \
     /aws-iam-authenticator
COPY --from=aws-iam-authenticator-build \
     /usr/share/licenses/aws-iam-authenticator \
     /licenses/aws-iam-authenticator

# eksctl
COPY --from=eksctl-build /home/builder/eksctl/eksctl /eksctl
COPY --from=eksctl-build /usr/share/licenses/eksctl /licenses/eksctl

# kubeadm and kubectl
COPY --from=kubernetes-build /home/builder/kubernetes/kubeadm /kubeadm
COPY --from=kubernetes-build /home/builder/kubernetes/kubectl /kubectl
COPY --from=kubernetes-build \
     /usr/share/licenses/kubernetes \
     /licenses/kubernetes

# sonobuoy
COPY --from=sonobuoy-build /home/builder/sonobuoy/sonobuoy /sonobuoy
COPY --from=sonobuoy-build /usr/share/licenses/sonobuoy /licenses/sonobuoy

# helm
COPY --from=helm-build /home/builder/helm/helm /helm
COPY --from=helm-build /usr/share/licenses/helm /licenses/helm
