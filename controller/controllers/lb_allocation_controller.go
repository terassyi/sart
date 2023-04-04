package controllers

import (
	"context"
	"fmt"
	"net/netip"
	"strings"

	"github.com/go-logr/logr"
	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/allocator"
	"github.com/terassyi/sart/controller/pkg/constants"
	v1 "k8s.io/api/core/v1"
	discoveryv1 "k8s.io/api/discovery/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/types"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/controller/controllerutil"
	"sigs.k8s.io/controller-runtime/pkg/handler"
	"sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/reconcile"
	"sigs.k8s.io/controller-runtime/pkg/source"
)

type LBAllocationReconciler struct {
	client.Client
	Allocators map[string]allocator.Allocator
}

// +kubebuilder:rbac:groups="",resources=services,verbs=get;list;watch;update;patch
// +kubebuilder:rbac:groups="",resources=services/status,verbs=get;update;patch
// +kubebuilder:rbac:groups="",resources=endpoints,verbs=get;list;watch;update;patch
// +kubebuilder:rbac:groups="",resources=endpoints/status,verbs=get;update;patch
// +kubebuilder:rbac:groups=discovery.k8s.io,resources=endpointslices,verbs=get;list;watch
// +kubebuilder:rbac:groups=discovery.k8s.io,resources=endpointslices/status,verbs=get;update;patch
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools,verbs=get;list;watch;update;patch;delete
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools/status,verbs=get;update;patch
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools/finalizers,verbs=update

func (r *LBAllocationReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	if _, err := r.reconcileAddressPool(ctx, req); err != nil {
		logger.Error(err, "failed to reconcile AddressPool")
		return ctrl.Result{}, err
	}

	if _, err := r.reconcileService(ctx, req); err != nil {
		logger.Error(err, "failed to reconcile Service")
		return ctrl.Result{}, err
	}

	return ctrl.Result{}, nil
}

func (r *LBAllocationReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&v1.Service{}).
		Watches(&source.Kind{Type: &sartv1alpha1.AddressPool{}}, &handler.EnqueueRequestForObject{}).
		Watches(&source.Kind{Type: &discoveryv1.EndpointSlice{}}, handler.EnqueueRequestsFromMapFunc(func(o client.Object) []reconcile.Request {
			epSlice, ok := o.(*discoveryv1.EndpointSlice)
			if !ok {
				return []reconcile.Request{}
			}
			serviceName, err := serviceNameFromEndpointSlice(epSlice)
			if err != nil {
				return []reconcile.Request{}
			}
			return []reconcile.Request{{NamespacedName: serviceName}}
		})).
		Complete(r)
}

func (r *LBAllocationReconciler) reconcileAddressPool(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	addressPool := &sartv1alpha1.AddressPool{}
	if err := r.Get(ctx, req.NamespacedName, addressPool); err != nil {
		if apierrors.IsNotFound(err) {
			return ctrl.Result{}, nil
		}
		logger.Error(err, "failed to get addresspool", "name", addressPool.Name)
		return ctrl.Result{}, err
	}

	// add finalizer
	finalizer := constants.GetFinalizerName(addressPool.TypeMeta)
	if !controllerutil.ContainsFinalizer(addressPool, finalizer) {
		controllerutil.AddFinalizer(addressPool, finalizer)
		if err := r.Client.Update(ctx, addressPool); err != nil {
			return ctrl.Result{}, err
		}
	}
	// remove finalizer and delete addresspool
	if !addressPool.ObjectMeta.DeletionTimestamp.IsZero() {
		logger.Info("deletion timestamp is not zero. remove AddressPool", "Name", addressPool.Name, "DeletionTimestamp", addressPool.DeletionTimestamp)
		if controllerutil.ContainsFinalizer(addressPool, finalizer) {
			controllerutil.RemoveFinalizer(addressPool, finalizer)

			delete(r.Allocators, addressPool.Name)

			if err := r.Client.Update(ctx, addressPool); err != nil {
				logger.Error(err, "failed to remove finalizer")
				return ctrl.Result{}, err
			}
		}
		return ctrl.Result{}, nil
	}

	if addressPool.Spec.Type != "lb" {
		return ctrl.Result{}, nil
	}

	p, ok := r.Allocators[addressPool.Name]
	if !ok {
		// create AddressaddressPool
		cidr, err := netip.ParsePrefix(addressPool.Spec.Cidr)
		if err != nil {
			logger.Error(err, "failed to parse CIDR", "AddressaddressPool.Spec.Cidr", addressPool.Spec.Cidr)
			return ctrl.Result{}, err
		}
		logger.Info("create an allocator", "CIDR", addressPool.Spec.Cidr, "Disabled", addressPool.Spec.Disable)
		r.Allocators[addressPool.Name] = allocator.New(&cidr)
		return ctrl.Result{}, nil
	}

	logger.Info("AddressaddressPool is already created", "AddressaddressPool", addressPool.Name, "allocator", p)
	if p.IsEnabled() == addressPool.Spec.Disable {
		if p.IsEnabled() {
			p.Disable()
		} else {
			p.Enable()
		}
	}
	return ctrl.Result{}, nil
}

func (r *LBAllocationReconciler) reconcileService(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	svc := &v1.Service{}
	if err := r.Client.Get(ctx, req.NamespacedName, svc); err != nil {
		if apierrors.IsNotFound(err) {
			// handle deletion
			return ctrl.Result{}, nil
		}
		logger.Error(err, "failed to get Service", "Name", req.NamespacedName)
		return ctrl.Result{}, err
	}

	// filter Load balancer type
	if svc.Spec.Type != v1.ServiceTypeLoadBalancer {
		return ctrl.Result{}, nil
	}
	// list endpoint slices
	epSliceList := discoveryv1.EndpointSliceList{}
	if err := r.Client.List(ctx, &epSliceList, client.MatchingLabels{constants.KubernetesServiceNameLabel: req.Name}); err != nil {
		logger.Error(err, "failed to list EndpointSlice by Service", "ServiceName", req.NamespacedName.String())
		return ctrl.Result{}, err
	}
	epNames := []string{}
	for _, eps := range epSliceList.Items {
		epNames = append(epNames, eps.Name)
	}
	logger.Info("endpoint slice list", "EndpointSlice", epNames)

	// allocation
	if err := r.allocate(ctx, svc, epSliceList); err != nil {
		logger.Error(err, "failed to allocate a LB address", "Name", req.NamespacedName)
		return ctrl.Result{}, err
	}

	return ctrl.Result{}, nil
}

func (r *LBAllocationReconciler) allocate(ctx context.Context, svc *v1.Service, eps discoveryv1.EndpointSliceList) error {
	logger := log.FromContext(ctx)

	// check protocol by clusterIP
	if len(svc.Spec.ClusterIP) == 0 && svc.Spec.ClusterIP == "" {
		return fmt.Errorf("Service doesn't have ClusterIP")
	}

	lbIPs := []netip.Addr{}
	// check allocated LB IP
	for _, ingress := range svc.Status.LoadBalancer.Ingress {
		addr, err := netip.ParseAddr(ingress.IP)
		if err != nil {
			return err
		}
		lbIPs = append(lbIPs, addr)
	}

	desiredIPs := make(map[string]netip.Addr)
	if len(lbIPs) > 0 {
		logger.Info("LB address is already allocated", "Name", svc.Name, "Addresses", lbIPs)
		poolName, ok := svc.Annotations[constants.AnnotationAddressPool]
		if !ok {
			return fmt.Errorf("required annotation %s is not found to address allocated load balancer", constants.AnnotationAddressPool)
		}
		from, ok := svc.Annotations[constants.AnnotationAllocatedFromPool]
		if !ok {
			return fmt.Errorf("required annotation %s is not found to address allocated load balancer", constants.AnnotationAllocatedFromPool)
		}
		if poolName != from {
			return fmt.Errorf("required pool is %s, actual allocating pool is %s", poolName, from)
		}

		a, ok := r.Allocators[poolName]
		if !ok {
			return fmt.Errorf("Allocator is not registered: %s", poolName)
		}
		for _, ip := range lbIPs {
			if a.IsAllocated(ip) {
				// already allocated, nothing to do
				return nil
			}
			// specified lb address, but not allocated
			if ip.Is4() {
				desiredIPs[constants.ProtocolIpv4] = ip
			} else {
				desiredIPs[constants.ProtocolIpv6] = ip
			}
		}
	}

	// get desired address from service spec
	// I'm not sure when dual stack
	if svc.Spec.LoadBalancerIP != "" {
		ipStrs := strings.Split(svc.Spec.LoadBalancerIP, ",")
		for _, is := range ipStrs {
			ip, err := netip.ParseAddr(is)
			if err != nil {
				return err
			}
			if ip.Is4() {
				desiredIPs[constants.ProtocolIpv4] = ip
			} else {
				desiredIPs[constants.ProtocolIpv6] = ip
			}
		}
	}

	// auto allocation

	return nil
}

func (r *LBAllocationReconciler) release(logger logr.Logger, svc *v1.Service) error {
	return nil
}

func serviceNameFromEndpointSlice(epSlice *discoveryv1.EndpointSlice) (types.NamespacedName, error) {
	if epSlice == nil {
		return types.NamespacedName{}, fmt.Errorf("EndpointSlice is nil")
	}
	svcName, ok := epSlice.Labels[discoveryv1.LabelServiceName]
	if !ok || svcName == "" {
		return types.NamespacedName{}, fmt.Errorf("ServiceName is not found in labels")
	}
	return types.NamespacedName{Namespace: epSlice.Namespace, Name: svcName}, nil
}
