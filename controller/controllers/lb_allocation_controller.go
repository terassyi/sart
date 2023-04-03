package controllers

import (
	"context"
	"net/netip"

	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/allocator"
	"github.com/terassyi/sart/controller/pkg/constants"
	v1 "k8s.io/api/core/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/controller/controllerutil"
	"sigs.k8s.io/controller-runtime/pkg/handler"
	"sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/source"
)

type LBAllocationReconciler struct {
	client.Client
	Allocators map[string]allocator.Allocator
}

// +kubebuilder:rbac:groups="",resources=services,verbs=get;list;watch;update;patch
// +kubebuilder:rbac:groups="",resources=services/status,verbs=get;update;patch
//+kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools,verbs=get;list;watch;update;patch;delete
//+kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools/status,verbs=get;update;patch
//+kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools/finalizers,verbs=update

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
			return ctrl.Result{}, nil
		}
		logger.Error(err, "failed to get Service", "Name", req.NamespacedName)
		return ctrl.Result{}, err
	}
	return ctrl.Result{}, nil
}
