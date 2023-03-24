package controllers

import (
	"context"
	"errors"

	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/speaker"
	v1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/runtime"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/log"
)

// BGPAdvertisementReconciler reconciles a BGPAdvertisement object
type BGPAdvertisementReconciler struct {
	client.Client
	Scheme              *runtime.Scheme
	SpeakerEndpointPort uint32
	SpeakerType         speaker.SpeakerType
}

// +kubebuilder:rbac:groups=sart.terassyi.net,resources=bgpadvertisements,verbs=get;list;watch;create;update;patch;delete
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=bgpadvertisements/status,verbs=get;update;patch
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=bgpadvertisements/finalizers,verbs=update
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=nodebgps,verbs=get;list

func (r *BGPAdvertisementReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	advertisement := &sartv1alpha1.BGPAdvertisement{}
	if err := r.Client.Get(ctx, req.NamespacedName, advertisement); err != nil {
		logger.Error(err, "failed to get resource", "BGPAdvertisement", req.NamespacedName)
		return ctrl.Result{}, err
	}

	peerList := &sartv1alpha1.BGPPeerList{}
	if err := r.Client.List(ctx, peerList); err != nil {
		logger.Error(err, "failed to list BGPPeer resource")
		return ctrl.Result{}, err
	}

	logger.Info("advertise based on ServiceType", "ServiceType", advertisement.Spec.ServiceType)
	notComplete := false
	switch advertisement.Spec.ServiceType {
	case v1.ServiceExternalTrafficPolicyTypeCluster:
		for _, p := range peerList.Items {
			if err := r.advertise(ctx, &p, advertisement); err != nil {
				if errors.Is(err, ErrPeerIsNotEstablished) {
					notComplete = true
					continue
				}
				return ctrl.Result{}, err
			}
		}
	case v1.ServiceExternalTrafficPolicyTypeLocal:
		for _, p := range peerList.Items {
			for _, target := range advertisement.Spec.Nodes {
				if p.Spec.Node == target {
					if err := r.advertise(ctx, &p, advertisement); err != nil {
						if errors.Is(err, ErrPeerIsNotEstablished) {
							notComplete = true
							continue
						}
						return ctrl.Result{}, err
					}
				}
			}
		}
	}

	if notComplete {
		advertisement.Status = sartv1alpha1.BGPAdvertisementStatusAdvertising
	} else {
		advertisement.Status = sartv1alpha1.BGPAdvertisementStatusNotAdvertised
	}

	if err := r.Client.Status().Update(ctx, advertisement); err != nil {
		logger.Error(err, "failed to update BGPAdvertisement status", "BGPAdvertisement", advertisement.Spec)
		return ctrl.Result{}, err
	}
	if advertisement.Status == sartv1alpha1.BGPAdvertisementStatusAdvertising {
		return ctrl.Result{Requeue: true}, nil // TODO: should we use RequeueAfter?
	}

	return ctrl.Result{}, nil
}

func (r *BGPAdvertisementReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&sartv1alpha1.BGPAdvertisement{}).
		Complete(r)
}

func (r *BGPAdvertisementReconciler) advertise(ctx context.Context, peer *sartv1alpha1.BGPPeer, adv *sartv1alpha1.BGPAdvertisement) error {
	logger := log.FromContext(ctx)

	speakerEndpoint := speaker.New(r.SpeakerType, peer.Spec.PeerRouterId, r.SpeakerEndpointPort)
	peerInfo, err := speakerEndpoint.GetPeer(ctx, peer.Spec.PeerRouterId)
	if err != nil {
		logger.Error(err, "failed to get peer information", "Peer", peer.Spec)
		return err
	}
	if peerInfo.State != speaker.PeerStateEstablished {
		return ErrPeerIsNotEstablished
	}
	if err := speakerEndpoint.AddPath(ctx, adv.ToPathInfo()); err != nil {
		logger.Error(err, "failed to add path to speaker", "Peer", peer.Spec, "Advertisement", adv.Spec)
		return err
	}
	logger.Info("advertise the path", "Path", adv.ToPathInfo())

	return nil
}

func (r *BGPAdvertisementReconciler) withdraw(ctx context.Context, peer *sartv1alpha1.BGPPeer) error {
	return nil
}
