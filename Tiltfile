load('ext://restart_process', 'docker_build_with_restart')

def crd():
	return 'make generate-crd'

def compile():
	return 'make build-daemon'

local_resource("Generate CRD manifests", crd(), deps=["sartd/src/controller/resources"])

local_resource("Deploy CRD", 'kubectl apply -f sartd/manifests/crd/crd.yaml', deps=["sartd/src/controller/resources"])

watch_file('sartd/manifests/sample')
k8s_yaml(kustomize('sartd/manifests/sample'))

local_resource(
	'Watch and Complie controller',
	compile(),
	deps=["sartd/src/controller"]
)

docker_build_with_restart(
	'localhost:5005/sart:dev',
	'.',
	dockerfile='Dockerfile.dev',
	entrypoint=['/usr/local/bin/sartd'],
	live_update=[
		sync('sartd/target/debug/sartd', '/usr/local/bin/sartd')
	]
)

local_resource("Deploy sample resources", 'kustomize build sartd/manifests/sample | kubectl apply -f -', deps=["sartd/manifests/sample"])
