.PHONY: setup
setup:
	sudo curl -Lo /usr/bin/tinet https://github.com/tinynetwork/tinet/releases/download/v0.0.2/tinet
	sudo chmod +x /usr/bin/tinet
	tinet --version


SPEC = basic
.PHONY: tinet
tinet.%:
	tinet ${@:tinet.%=%} -c $(SPEC)/spec.yaml | sudo sh -x


ROUTER = R0
IFACE = net0

.PHONY: capture
capture:
	tinet down -c $(SPEC)/spec.yaml | sudo sh -x
	tinet up -c $(SPEC)/spec.yaml | sudo sh -x
	docker exec -it $(ROUTER) tcpdump -i $(IFACE) -w /tmp/$(SPEC)/dump.pcap &
	tinet conf -c $(SPEC)/spec.yaml | sudo sh -x
	
