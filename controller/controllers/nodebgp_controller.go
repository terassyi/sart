/*
Copyright 2023.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

package controllers

import (
	"context"
	"time"

	apierrors "k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/runtime"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/log"

	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/speaker"
)

// NodeBGPReconciler reconciles a NodeBGP object
type NodeBGPReconciler struct {
	client.Client
	Scheme              *runtime.Scheme
	SpeakerEndpointPort uint32
	SpeakerType         speaker.SpeakerType
}

//+kubebuilder:rbac:groups=sart.terassyi.net,resources=nodebgps,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=sart.terassyi.net,resources=nodebgps/status,verbs=get;update;patch
//+kubebuilder:rbac:groups=sart.terassyi.net,resources=nodebgps/finalizers,verbs=update

// Reconcile is part of the main kubernetes reconciliation loop which aims to
// move the current state of the cluster closer to the desired state.
// TODO(user): Modify the Reconcile function to compare the state specified by
// the NodeBGP object against the actual cluster state, and then
// perform operations to make the cluster state reflect the state specified by
// the user.
//
// For more details, check Reconcile and its Result here:
// - https://pkg.go.dev/sigs.k8s.io/controller-runtime@v0.14.1/pkg/reconcile
func (r *NodeBGPReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	// get NodeBGP resources
	nodeBgp := &sartv1alpha1.NodeBGP{}
	if err := r.Client.Get(ctx, req.NamespacedName, nodeBgp); err != nil {
		if apierrors.IsNotFound(err) {
			return ctrl.Result{}, nil
		}
		return ctrl.Result{}, err
	}

	newNodeBgp := nodeBgp.DeepCopy()
	speakerEndpoint := speaker.New(r.SpeakerType, nodeBgp.Spec.RouterId, r.SpeakerEndpointPort)
	info, err := speakerEndpoint.GetInfo(ctx)
	if err != nil {
		logger.Error(err, "failed to get speaker info", "NodeBGP", nodeBgp.Spec.RouterId)
		newNodeBgp.Status = sartv1alpha1.NodeBGPStatusUnavailable
		if err := r.Client.Status().Patch(ctx, newNodeBgp, client.MergeFrom(nodeBgp)); err != nil {
			return ctrl.Result{}, err
		}
		return ctrl.Result{Requeue: true, RequeueAfter: time.Second * 5}, nil
	}

	if nodeBgp.Spec.Asn == info.Asn && nodeBgp.Spec.RouterId == info.RouterId && nodeBgp.Status == sartv1alpha1.NodeBGPStatusAvailable {
		return ctrl.Result{}, nil
	}
	if nodeBgp.Spec.Asn != info.Asn || nodeBgp.Spec.RouterId != info.RouterId {
		logger.Info("don't match NodeBGP info", "ASN", info.Asn, "RouerId", info.RouterId)
		if err := speakerEndpoint.SetInfo(ctx, speaker.SpeakerInfo{
			Asn:      nodeBgp.Spec.Asn,
			RouterId: nodeBgp.Spec.RouterId,
		}); err != nil {
			logger.Error(err, "failed to set NodeBGP information", "NodeBGP", nodeBgp.Name)
			return ctrl.Result{}, err
		}
	}

	newNodeBgp.Status = sartv1alpha1.NodeBGPStatusAvailable
	if err := r.Client.Status().Patch(ctx, newNodeBgp, client.MergeFrom(nodeBgp)); err != nil {
		return ctrl.Result{}, err
	}

	return ctrl.Result{}, nil
}

// SetupWithManager sets up the controller with the Manager.
func (r *NodeBGPReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&sartv1alpha1.NodeBGP{}).
		Complete(r)
}

func isPeerRegistered(ctx context.Context, nodeBgp *sartv1alpha1.NodeBGP, peer *sartv1alpha1.BGPPeer) (int, bool, bool) {
	index := 0
	for i, p := range nodeBgp.Spec.Peers {
		index = i
		if peer.Name == p.Name && peer.Namespace == p.Namespace {
			if p.Asn == peer.Spec.PeerAsn && p.RouterId == peer.Spec.PeerRouterId && p.Status == peer.Status {
				return index, true, false
			}
			return index, true, true
		}
	}
	return index, false, false
}
