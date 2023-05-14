package controllers

import (
	"context"
	"fmt"
	"strconv"

	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/constants"
	"github.com/terassyi/sart/controller/pkg/speaker"
	v1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/handler"
	"sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/source"
)

type NodeWatcher struct {
	client.Client
	SpeakerEndpointPort uint32
	SpeakerType         speaker.SpeakerType
}

// +kubebuilder:rbac:groups="",resources=nodes,verbs=get;list;watch
// +kubebuilder:rbac:groups="",resources=nodes/status,verbs=get;update;patch
// +kubebuilder:rbac:groups="sart.terassyi.net",resources=nodebgps,verbs=get;list;watch;create;delete
// +kubebuilder:rbac:groups="sart.terassyi.net",resources=clusterbgps,verbs=get;list;update

func (n *NodeWatcher) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	clusterBGPs := sartv1alpha1.ClusterBGPList{}
	if err := n.Client.List(ctx, &clusterBGPs); err != nil {
		return ctrl.Result{}, err
	}
	if len(clusterBGPs.Items) == 0 {
		return ctrl.Result{}, nil
	} else if len(clusterBGPs.Items) > 1 {
		return ctrl.Result{}, fmt.Errorf("a ClusterBGP resource must be one")
	}
	clusterBGP := clusterBGPs.Items[0]

	// policy := clusterBGP.Spec.PeeringPolicy

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

		exist := false
		for _, nodeBgp := range nodeBgpList.Items {
			addr := nodeBgp.Spec.RouterId
			if func() bool {
				for _, a := range addrs {
					if a.Address == addr {
						return true
					}
				}
				return false
			}() {
				exist = true
				break
			}
		}
		if exist {
			continue
		}
		// if a NodeBGP resource is not created for Node
		internalIp, ok := getNodeInternalIp(&node)
		if !ok {
			return ctrl.Result{}, fmt.Errorf("internal IP is not found in %s", node.Name)
		}
		nodeEndpoint := speaker.New(n.SpeakerType, internalIp, n.SpeakerEndpointPort)
		if err := nodeEndpoint.HealthCheck(ctx); err != nil {
			logger.Error(err, "failed to communicate to speaker", "Node", node.Name, "NodeIP", internalIp)
			return ctrl.Result{Requeue: true}, err
		}

		// get asn from node's annotation
		// TODO: select asn selection policy by ClusterBGP.Spec.PeeringPolicy
		s, ok := node.Labels[constants.LabelKeyAsn]
		if !ok {
			logger.Info("ignore the node doesn't has an ASN annotation")
			return ctrl.Result{}, nil
		}
		asn, err := strconv.ParseUint(s, 10, 64)
		if err != nil {
			logger.Error(err, "failed to parse asn from label", "Node", node.Name, "Value", s)
			return ctrl.Result{}, err
		}

		nodeBgp := &sartv1alpha1.NodeBGP{
			ObjectMeta: metav1.ObjectMeta{
				Name:      node.Name,
				Namespace: constants.Namespace,
			},
			Spec: sartv1alpha1.NodeBGPSpec{
				Asn:      uint32(asn),
				RouterId: internalIp,
			},
		}
		logger.Info("create resource", "NodeBGP", nodeBgp)
		if err := n.Client.Create(ctx, nodeBgp); err != nil {
			return ctrl.Result{}, err
		}
		clusterBGP.Spec.Nodes = append(clusterBGP.Spec.Nodes, *nodeBgp)
	}

	// delete NodeBGP if node is removed
	for _, nodeBgp := range nodeBgpList.Items {
		exist := false
		for _, node := range nodeList.Items {
			if func() bool {
				for _, a := range node.Status.Addresses {
					if a.Address == nodeBgp.Spec.RouterId {
						return true
					}
				}
				return false
			}() {
				exist = true
				break
			}
		}
		if !exist {
			if err := n.Client.Delete(ctx, &nodeBgp); err != nil {
				return ctrl.Result{}, err
			}
		}
	}

	return ctrl.Result{}, nil
}

func (n *NodeWatcher) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&v1.Node{}).
		Watches(&source.Kind{Type: &sartv1alpha1.ClusterBGP{}}, &handler.EnqueueRequestForObject{}).
		Complete(n)
}

func getNodeInternalIp(node *v1.Node) (string, bool) {
	for _, addr := range node.Status.Addresses {
		if addr.Type == v1.NodeInternalIP {
			return addr.Address, true
		}
	}
	return "", false
}
