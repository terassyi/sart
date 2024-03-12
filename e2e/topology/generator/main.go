package main

import (
	"context"
	"flag"
	"io"
	"os"
	"path/filepath"
	"text/template"

	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/client-go/kubernetes"
	"k8s.io/client-go/tools/clientcmd"
)

const (
	TOPOLOGY_CONFIG_TMPL string = "kubernetes.yaml.tmpl"
	TOPOLOGY_CONFIG      string = "kubernetes.yaml"
)

type NodeAddrs struct {
	ControlPlane        string
	Worker              string
	Worker2             string
	Worker3             string
	ControlPlanePodCIDR string
	WorkerPodCIDR       string
	Worker2PodCIDR      string
	Worker3PodCIDR      string
}

var (
	tmplPath   = flag.String("template-path", TOPOLOGY_CONFIG_TMPL, "path to template")
	outputPath = flag.String("output-path", TOPOLOGY_CONFIG, "path to output")
)

func main() {

	flag.Parse()

	homeDir := os.Getenv("HOME")
	kubeConfigPath := filepath.Join(homeDir, ".kube/config")
	config, err := clientcmd.BuildConfigFromFlags("", kubeConfigPath)
	if err != nil {
		panic(err)
	}
	clientset, err := kubernetes.NewForConfig(config)
	if err != nil {
		panic(err)
	}
	ctx := context.Background()
	nodes, err := clientset.CoreV1().Nodes().List(ctx, metav1.ListOptions{})
	if err != nil {
		panic(err)
	}
	var nodeAddrs NodeAddrs
	for _, node := range nodes.Items {
		switch node.GetName() {
		case "sart-control-plane":
			for _, statusAddrs := range node.Status.Addresses {
				if statusAddrs.Type == "InternalIP" {
					nodeAddrs.ControlPlane = statusAddrs.Address
				}
			}
			nodeAddrs.ControlPlanePodCIDR = node.Spec.PodCIDR
		case "sart-worker":
			for _, statusAddrs := range node.Status.Addresses {
				if statusAddrs.Type == "InternalIP" {
					nodeAddrs.Worker = statusAddrs.Address
				}
			}
			nodeAddrs.WorkerPodCIDR = node.Spec.PodCIDR
		case "sart-worker2":
			for _, statusAddrs := range node.Status.Addresses {
				if statusAddrs.Type == "InternalIP" {
					nodeAddrs.Worker2 = statusAddrs.Address
				}
			}
			nodeAddrs.Worker2PodCIDR = node.Spec.PodCIDR
		case "sart-worker3":
			for _, statusAddrs := range node.Status.Addresses {
				if statusAddrs.Type == "InternalIP" {
					nodeAddrs.Worker3 = statusAddrs.Address
				}
			}
			nodeAddrs.Worker3PodCIDR = node.Spec.PodCIDR
		default:
		}
	}

	if !nodeAddrs.check() {
		panic("Failed to get Node Address")
	}

	file, err := os.Open(*tmplPath)
	if err != nil {
		panic(err)
	}
	defer file.Close()

	tmplData, err := io.ReadAll(file)
	if err != nil {
		panic(err)
	}

	tmpl, err := template.New(TOPOLOGY_CONFIG_TMPL).Parse(string(tmplData))
	if err != nil {
		panic(err)
	}

	outFile, err := os.Create(*outputPath)
	if err != nil {
		panic(err)
	}
	defer outFile.Close()

	if err := tmpl.Execute(outFile, nodeAddrs); err != nil {
		panic(err)
	}

}

func (n *NodeAddrs) check() bool {
	return !(n.ControlPlane == "" || n.Worker == "" || n.Worker2 == "" || n.Worker3 == "")
}
