RUSTUP := rustup
CARGO := cargo
GOBGP_VERSION := 3.10.0
GRPCURL_VERSION := 1.8.7
IMAGE_VERSION := dev
PROJECT := github.com/terassyi/sart

RELEASE_VERSION=
.PHONY: release-pr
release-pr: validate-release-version cargo-bump

	git checkout main
	git pull origin main
	git checkout -b bump-v$(RELEASE_VERSION)

	yq -i '.images[].newTag="$(RELEASE_VERSION)"' controller/config/release/kustomization.yaml
	cd sartd; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sart; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update

	$(MAKE) build-image
	cd controller; make docker-build

	git add .
	git commit -m "bump v$(RELEASE_VERSION)" --allow-empty

	gh pr create --base main --title "Bump v$(RELEASE_VERSION)" --body ""

.PHONY: release
release: validate-release-version
	git checkout main
	git pull origin main

ifeq ($(shell git log --pretty=format:"%s" -1 --grep="$(RELEASE_VERSION)"),)
	echo "Leatest commit is not release PR"
	exit 1
endif

	git tag -a -m "Release v$(RELEASE_VERSION)" "v$(RELEASE_VERSION)"
	git tag -ln | grep v$(RELEASE_VERSION)
	git push origin "v$(RELEASE_VERSION)"

validate-release-version:
ifndef RELEASE_VERSION
	echo "Please specify a release version"
	exit 1
endif



.PHONY: setup
setup: setup-rust-tools setup-grpc

.PHONY: setup-rust-tools
setup-rust-tools:
	$(RUSTUP) component add llvm-tools-preview
	$(RUSTUP) component add rustfmt
	$(CARGO) install rustfilt

.PHONY: setup-grpc
setup-grpc:
	sudo apt install -y protobuf-compiler libprotobuf-dev
	go install github.com/bufbuild/buf/cmd/buf@latest
	go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
	go install github.com/bufbuild/connect-go/cmd/protoc-gen-connect-go@latest
	go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@latest

.PHONY: setup-dev
setup-dev:
	sudo apt install -y frr jq iproute2
	sudo wget -P /tmp https://github.com/osrg/gobgp/releases/download/v${GOBGP_VERSION}/gobgp_${GOBGP_VERSION}_linux_amd64.tar.gz
	sudo tar -zxvf /tmp/gobgp_${GOBGP_VERSION}_linux_amd64.tar.gz -C /usr/bin/
	sudo wget -P /tmp https://github.com/fullstorydev/grpcurl/releases/download/v${GRPCURL_VERSION}/grpcurl_${GRPCURL_VERSION}_linux_x86_64.tar.gz
	sudo tar -zxvf /tmp/grpcurl_${GRPCURL_VERSION}_linux_x86_64.tar.gz -C /usr/bin/

.PHONY: release-build
release-build:
	cd sartd; $(CARGO) build --release
	cd ..
	cd sart; $(CARGO) build --release

.PHONY: build
build: build-daemon build-cli

.PHONY: build-daemon
build-daemon:
	cd sartd; $(CARGO) build --verbose

.PHONY: build-cli
build-cli:
	cd sart; $(CARGO) build --verbose

.PHONY: build-proto
build-proto:
	protoc -Iproto --go_out=./controller/pkg/proto bgp.proto
	protoc -Iproto --go-grpc_out=./controller/pkg/proto bgp.proto
	protoc -Iproto --go_out=./controller/pkg/proto fib.proto
	protoc -Iproto --go-grpc_out=./controller/pkg/proto fib.proto


.PHONY: fmt
fmt:
	cd sartd; $(CARGO) fmt

.PHONY: test
test: unit-test controller-test e2e-test

.PHONY: unit-test
unit-test:
	cd sartd; $(CARGO) test

.PHONY: e2e-test
e2e-test: sart-e2e-test controller-e2e-test

.PHONY: sart-e2e-test
sart-e2e-test:
	cd e2e; go test -v ./sartd

.PHONY: controller-e2e-test
controller-e2e-test:
	cd e2e/controller; make start
	cd e2e/controller; make install
	cd e2e/controller; make test
	cd e2e/controller; make stop

.PHONY: controller-test
controller-test:
	cd controller; make test

.PHONY: dev-container
dev-container:
	docker run -it --privileged --rm --name sart-dev -p 8080:8080 -w /work/sart -v `pwd`:/work/sart ghcr.io/terassyi/terakoya:0.1.2 bash

.PHONY: in-container
in-container:
	docker exec -it sart-dev bash

.PHONY: build-image
build-image:
	docker build -t sart:${IMAGE_VERSION} .

.PHONY: build-dev-image
build-dev-image:
	docker build -t sart:${IMAGE_VERSION} -f Dockerfile.dev .


CERT_MANAGER_VERSION := 1.11.2
BUILD ?= false
REGISTORY_URL ?= localhost:5005
DEVENV_BGP_ASN ?= 65000
NODE0_ASN ?= 65000
NODE1_ASN ?= 65000
NODE2_ASN ?= 65000
NODE3_ASN ?= 65000
DEVENV_BGP_ADDR ?= ""
NODE0_ADDR ?= ""
NODE1_ADDR ?= ""
NODE2_ADDR ?= ""
NODE3_ADDR ?= ""
CLIENT_ADDR ?= ""
LB_CIDR ?= 10.69.0.0/24
ESCAPED_LB_CIDR ?= "10.69.0.0\/24"

.PHONY: devenv
devenv:
	${SUDO} sysctl -w fs.inotify.max_user_instances=512
	${SUDO} sysctl -w fs.inotify.max_user_watches=65536
	if [ ${BUILD} = "daemon" ]; then \
		make build-dev-image; \
	elif [ ${BUILD} = "controller" ]; then \
		cd controller; make docker-build; \
	elif [ ${BUILD} = "all" ]; then \
		make build-image; \
		cd controller; make docker-build; \
	fi
	docker rm -f devenv-bgp || true
	docker rm -f client || true

	rm -f ./devenv/frr/staticd.conf || true

	ctlptl apply -f ./controller/ctlptl.yaml
	$(eval REGISTORY_URL = $(shell ctlptl get cluster kind-devenv -o template --template '{{.status.localRegistryHosting.host}}'))
	make push-image

	kubectl label nodes --overwrite devenv-control-plane sart.terassyi.net/asn=${NODE0_ASN}
	kubectl label nodes --overwrite devenv-worker sart.terassyi.net/asn=${NODE1_ASN}
	kubectl label nodes --overwrite devenv-worker2 sart.terassyi.net/asn=${NODE2_ASN}
	kubectl apply -f https://github.com/jetstack/cert-manager/releases/download/v$(CERT_MANAGER_VERSION)/cert-manager.yaml
	kubectl -n cert-manager wait --for=condition=available --timeout=180s --all deploymentskubectl label nodes --overwrite devenv-worker3 sart.terassyi.net/asn=${NODE3_ASN}

	$(eval NODE0_ADDR = $(shell kubectl get nodes devenv-control-plane -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE1_ADDR = $(shell kubectl get nodes devenv-worker -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE2_ADDR = $(shell kubectl get nodes devenv-worker2 -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE3_ADDR = $(shell kubectl get nodes devenv-worker3 -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))

	docker run -d --privileged --network kind  --rm --ulimit core=-1 --name devenv-bgp --volume `pwd`/devenv/frr:/etc/frr/ ghcr.io/terassyi/terakoya:0.1.2 tail -f /dev/null
	docker run -d --privileged --network kind --rm --name client ghcr.io/terassyi/terakoya:0.1.2 tail -f /dev/null

	make configure-bgp


.PHONY: configure-bgp
configure-bgp:

	docker exec devenv-bgp /usr/lib/frr/frrinit.sh start

	$(eval DEVENV_BGP_ADDR = $(shell docker inspect devenv-bgp | jq '.[0].NetworkSettings.Networks.kind.IPAddress' | tr -d '"'))
	$(eval NODE0_ADDR = $(shell kubectl get nodes devenv-control-plane -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE1_ADDR = $(shell kubectl get nodes devenv-worker -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE2_ADDR = $(shell kubectl get nodes devenv-worker2 -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE3_ADDR = $(shell kubectl get nodes devenv-worker3 -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	sed -e s/NODE0_ASN/${NODE0_ASN}/g -e s/NODE1_ASN/${NODE1_ASN}/g -e s/NODE2_ASN/${NODE2_ASN}/g -e s/NODE3_ASN/${NODE3_ASN}/g \
		-e s/DEVENV_BGP_ASN/${DEVENV_BGP_ASN}/g \
		-e s/DEVENV_BGP_ADDR/${DEVENV_BGP_ADDR}/g \
		-e s/NODE0_ADDR/${NODE0_ADDR}/g -e s/NODE1_ADDR/${NODE1_ADDR}/g -e s/NODE2_ADDR/${NODE2_ADDR}/g -e s/NODE3_ADDR/${NODE3_ADDR}/g \
		./devenv/frr/gobgp.conf.tmpl > ./devenv/frr/gobgp.conf


	sed -e s/LB_CIDR/'${ESCAPED_LB_CIDR}'/g \
		./controller/config/sample_templates/_v1alpha1_addresspool.yaml.tmpl > ./controller/config/samples/_v1alpha1_addresspool.yaml

	sed -e s/DEVENV_BGP_ASN/${DEVENV_BGP_ASN}/g -e s/DEVENV_BGP_ADDR/${DEVENV_BGP_ADDR}/g \
		./controller/config/sample_templates/_v1alpha1_bgppeer.yaml.tmpl > ./controller/config/samples/_v1alpha1_bgppeer.yaml

	docker exec -d devenv-bgp gobgpd -f /etc/frr/gobgp.conf
	docker exec client ip route add ${LB_CIDR} via ${DEVENV_BGP_ADDR}

.PHONY:
push-image:
	docker image tag sart:${IMAGE_VERSION} ${REGISTORY_URL}/sart:${IMAGE_VERSION}
	docker image tag sart-controller:${IMAGE_VERSION} ${REGISTORY_URL}/sart-controller:${IMAGE_VERSION}
	docker image tag test-app:${IMAGE_VERSION} ${REGISTORY_URL}/test-app:${IMAGE_VERSION}
	docker push ${REGISTORY_URL}/sart:${IMAGE_VERSION}
	docker push ${REGISTORY_URL}/sart-controller:${IMAGE_VERSION}
	docker push ${REGISTORY_URL}/test-app:${IMAGE_VERSION}

.PHONY: clean-devenv
clean-devenv:
	ctlptl delete -f ./controller/ctlptl.yaml
	docker rm -f devenv-bgp
	docker rm -f client
	rm -f ./devenv/frr/staticd.conf || true

CARGO_BUMP ?= cargo-bump

.PHONY: cargo-bump
cargo-bump:
$(CARGO_BUMP):
ifeq ($(shell which ${CARGO_BUMP}),)
	cargo install cargo-bump
endif
