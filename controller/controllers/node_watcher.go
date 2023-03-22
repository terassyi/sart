package controllers

import (
	"context"

	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	v1 "k8s.io/api/core/v1"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/log"
)

type NodeWatcher struct {
	client.Client
	speakerHeathEndpointPort uint32
}

// +kubebuilder:rbac:groups="",resources=nodes,verbs=get;list;watch
// +kubebuilder:rbac:groups="",resources=nodes/status,verbs=get;update;patch

func (n *NodeWatcher) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)
	logger.Info("reconcile", "controller", "node_watcher")

	// watch nodes and confirm a bgp speaker is running on the host
	nodeList := &v1.NodeList{}
	if err := n.Client.List(ctx, nodeList); err != nil {
		return ctrl.Result{}, err
	}
	nodeBgpList := &sartv1alpha1.NodeBGPList{}
	if err := n.Client.List(ctx, nodeBgpList); err != nil {
		return ctrl.Result{}, err
	}

	for _, node := range nodeList.Items {
		addrs := node.Status.Addresses
		logger.Info("node addresses", "name", node.Name, "addresses", addrs)
	}
	return ctrl.Result{}, nil
}

func (n *NodeWatcher) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).For(&v1.Node{}).Complete(n)
}
