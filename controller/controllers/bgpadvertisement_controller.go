package controllers

import (
	"context"
	"errors"
	"fmt"
	"net"
	"time"

	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/speaker"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/types"
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
		if apierrors.IsNotFound(err) {
			return r.reconcileWhenDelete(ctx, req)
		}
		logger.Error(err, "failed to get resource", "BGPAdvertisement", req.NamespacedName)
		return ctrl.Result{}, err
	}

	switch advertisement.Status {
	case sartv1alpha1.BGPAdvertisementStatusAdvertising:
		return r.reconcileWhenAdvertising(ctx, advertisement)
	case sartv1alpha1.BGPAdvertisementStatusUpdated:
	case sartv1alpha1.BGPAdvertisementStatusAdvertised:
	case sartv1alpha1.BGPAdvertisementStatusWithdrawn:
	}
	return ctrl.Result{}, nil
}

func (r *BGPAdvertisementReconciler) reconcileWhenAdvertising(ctx context.Context, advertisement *sartv1alpha1.BGPAdvertisement) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	peerList := &sartv1alpha1.BGPPeerList{}
	if err := r.Client.List(ctx, peerList); err != nil {
		logger.Error(err, "failed to list BGPPeer resource")
		return ctrl.Result{}, err
	}

	notComplete := false
	for _, p := range peerList.Items {
		for _, target := range advertisement.Spec.Nodes {
			if p.Spec.Node == target {
				if p.Status != sartv1alpha1.BGPPeerStatusEstablished {
					logger.Info("BGPPeer is unavailable", "BGPPeer", p.Spec)
					return ctrl.Result{Requeue: true, RequeueAfter: time.Second * 5}, nil
				}
				if err := r.advertise(ctx, &p, advertisement); err != nil {
					if errors.Is(err, ErrPeerIsNotEstablished) {
						notComplete = true
						continue
					}
					return ctrl.Result{}, err
				}
				contain := false
				for _, adv := range p.Spec.Advertisements {
					if adv.Name == advertisement.Name {
						contain = true
						break
					}
				}
				if !contain {
					p.Spec.Advertisements = append(p.Spec.Advertisements, sartv1alpha1.Advertisement{
						Name:      advertisement.Name,
						Namespace: advertisement.Namespace,
						Prefix:    advertisement.Spec.Network,
					})
					if err := r.Client.Update(ctx, &p); err != nil {
						return ctrl.Result{}, err
					}
				}
			}
		}
	}

	if notComplete {
		advertisement.Status = sartv1alpha1.BGPAdvertisementStatusAdvertising
	} else {
		advertisement.Status = sartv1alpha1.BGPAdvertisementStatusAdvertised
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

func (r *BGPAdvertisementReconciler) reconcileWhenDelete(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	peerList := &sartv1alpha1.BGPPeerList{}
	if err := r.Client.List(ctx, peerList); err != nil {
		logger.Error(err, "failed to list BGPPeer")
		return ctrl.Result{}, err
	}
	// find advertisements
	for _, p := range peerList.Items {
		remove := 0
		needUpdate := false
		for i, adv := range p.Spec.Advertisements {
			if req.Name == adv.Name && req.Namespace == adv.Namespace {
				// withdraw
				if err := r.withdraw(ctx, &p, adv.Prefix); err != nil {
					logger.Error(err, "failed to withdraw path by speaker", "Advertisement", adv)
					return ctrl.Result{}, err
				}
				remove = i
				needUpdate = true
				break
			}
		}
		if needUpdate {
			p.Spec.Advertisements = append(p.Spec.Advertisements[:remove], p.Spec.Advertisements[remove+1:]...)
			if err := r.Client.Update(ctx, &p); err != nil {
				logger.Error(err, "failed to update peer", "Peer", p.Spec)
				return ctrl.Result{}, err
			}
		}
	}
	return ctrl.Result{}, nil
}

func (r *BGPAdvertisementReconciler) reconcileWhenUpdated(ctx context.Context, advertisement *sartv1alpha1.BGPAdvertisement) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	peerList := &sartv1alpha1.BGPPeerList{}
	if err := r.Client.List(ctx, peerList); err != nil {
		logger.Error(err, "failed to list BGPPeer")
		return ctrl.Result{}, err
	}
	// search advertised path
	added, removed := advDiff(*peerList, advertisement)

	addedNodes, removedNodes := []string{}, []string{}
	for _, n := range added {
		addedNodes = append(addedNodes, n.Spec.Node)
	}
	for _, n := range added {
		removedNodes = append(removedNodes, n.Spec.Node)
	}
	logger.Info("advertisement", "Name", types.NamespacedName{Namespace: advertisement.Namespace, Name: advertisement.Name}, "Added", addedNodes, "Removed", removedNodes)

	notComplete := false

	for _, p := range removed {
		logger.Info("withdraw from", "Node", p.Spec.Node, "Advertisement", advertisement.Name, "Prefix", advertisement.Spec.Network)
		if err := r.withdraw(ctx, &p, advertisement.Spec.Network); err != nil {
			if errors.Is(err, ErrPeerIsNotEstablished) {
				notComplete = true
				continue
			}
			logger.Error(err, "failed to withdraw path by speaker", "Advertisement", advertisement)
			return ctrl.Result{}, err
		}
		// remove from peer advertisement list
		index := -1
		for i, adv := range p.Spec.Advertisements {
			if adv.Name == advertisement.Name {
				index = i
				break
			}
		}
		if index != -1 {
			p.Spec.Advertisements = append(p.Spec.Advertisements[:index], p.Spec.Advertisements[index+1:]...)
			if err := r.Client.Update(ctx, &p); err != nil {
				return ctrl.Result{}, err
			}
		}

	}

	for _, p := range added {
		logger.Info("advertise to", "Node", p.Spec.Node, "Advertisement", advertisement.Name, "Prefix", advertisement.Spec.Network)
		if err := r.advertise(ctx, &p, advertisement); err != nil {
			if errors.Is(err, ErrPeerIsNotEstablished) {
				notComplete = true
				continue
			}
			return ctrl.Result{}, err
		}
		// append peer advertisement list
		contain := false
		for _, adv := range p.Spec.Advertisements {
			if adv.Name == advertisement.Name {
				contain = true
				break
			}
		}
		if !contain {
			p.Spec.Advertisements = append(p.Spec.Advertisements, sartv1alpha1.Advertisement{
				Name:      advertisement.Name,
				Namespace: advertisement.Namespace,
				Prefix:    advertisement.Spec.Network,
			})
			if err := r.Client.Update(ctx, &p); err != nil {
				return ctrl.Result{}, err
			}
		}
	}

	if notComplete {
		advertisement.Status = sartv1alpha1.BGPAdvertisementStatusAdvertising
	} else {
		advertisement.Status = sartv1alpha1.BGPAdvertisementStatusAdvertised
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

func (r *BGPAdvertisementReconciler) advertise(ctx context.Context, peer *sartv1alpha1.BGPPeer, adv *sartv1alpha1.BGPAdvertisement) error {
	logger := log.FromContext(ctx)

	speakerEndpoint := speaker.New(r.SpeakerType, peer.Spec.LocalRouterId, r.SpeakerEndpointPort)
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

func (r *BGPAdvertisementReconciler) withdraw(ctx context.Context, peer *sartv1alpha1.BGPPeer, prefix string) error {
	logger := log.FromContext(ctx)

	speakerEndpoint := speaker.New(r.SpeakerType, peer.Spec.LocalRouterId, r.SpeakerEndpointPort)
	peerInfo, err := speakerEndpoint.GetPeer(ctx, peer.Spec.PeerRouterId)
	if err != nil {
		logger.Error(err, "failed to get peer information", "Peer", peer.Spec)
		return err
	}
	if peerInfo.State != speaker.PeerStateEstablished {
		return ErrPeerIsNotEstablished
	}
	addr, _, err := net.ParseCIDR(prefix)
	if err != nil {
		return err
	}
	pathInfo := speaker.PathInfo{
		Prefix: prefix,
	}
	if addr.To4() != nil {
		pathInfo.Protocol = "ipv4"
	} else if addr.To16() != nil {
		pathInfo.Protocol = "ipv6"
	} else {
		return fmt.Errorf("invalid protocol format")
	}

	if err := speakerEndpoint.DeletePath(ctx, pathInfo); err != nil {
		return err
	}
	logger.Info("withdraw the path", "Prefix", prefix)
	return nil
}

func advDiff(peerList sartv1alpha1.BGPPeerList, advertisement *sartv1alpha1.BGPAdvertisement) ([]sartv1alpha1.BGPPeer, []sartv1alpha1.BGPPeer) {
	peerMap := make(map[string]sartv1alpha1.BGPPeer)
	oldMap := make(map[string]sartv1alpha1.BGPPeer)
	for _, peer := range peerList.Items {
		peerMap[peer.Spec.Node] = peer
		for _, adv := range peer.Spec.Advertisements {
			if adv.Namespace == advertisement.Namespace && adv.Name == advertisement.Name {
				// advertised from this peer
				oldMap[peer.Spec.Node] = peer
			}
		}
	}

	removed := []sartv1alpha1.BGPPeer{}
	added := []sartv1alpha1.BGPPeer{}

	for _, an := range advertisement.Spec.Nodes {
		_, ok := oldMap[an]
		if !ok {
			added = append(added, peerMap[an])
		} else {
			delete(oldMap, an)
		}
	}

	for _, p := range oldMap {
		removed = append(removed, p)
	}

	return added, removed

}
