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

package v1alpha1

import (
	"errors"

	"k8s.io/apimachinery/pkg/runtime"
	ctrl "sigs.k8s.io/controller-runtime"
	logf "sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/webhook"
)

// log is for logging in this package.
var nodebgplog = logf.Log.WithName("nodebgp-resource")

var NodeBGPOwnerReferenceNotFound error = errors.New("Owner reference by Node is not found")

func (r *NodeBGP) SetupWebhookWithManager(mgr ctrl.Manager) error {
	return ctrl.NewWebhookManagedBy(mgr).
		For(r).
		Complete()
}

// TODO(user): EDIT THIS FILE!  THIS IS SCAFFOLDING FOR YOU TO OWN!

// TODO(user): change verbs to "verbs=create;update;delete" if you want to enable deletion validation.
//+kubebuilder:webhook:path=/validate-sart-terassyi-net-v1alpha1-nodebgp,mutating=false,failurePolicy=fail,sideEffects=None,groups=sart.terassyi.net,resources=nodebgps,verbs=create;update,versions=v1alpha1,name=vnodebgp.kb.io,admissionReviewVersions=v1

var _ webhook.Validator = &NodeBGP{}

// ValidateCreate implements webhook.Validator so a webhook will be registered for the type
func (r *NodeBGP) ValidateCreate() error {
	nodebgplog.Info("validate create", "name", r.Name)
	if hasOwnerReferenceByNode(r) {
		return nil
	}
	return NodeBGPOwnerReferenceNotFound

}

// ValidateUpdate implements webhook.Validator so a webhook will be registered for the type
func (r *NodeBGP) ValidateUpdate(old runtime.Object) error {
	nodebgplog.Info("validate update", "name", r.Name)

	if hasOwnerReferenceByNode(r) {
		return nil
	}
	return NodeBGPOwnerReferenceNotFound
}

// ValidateDelete implements webhook.Validator so a webhook will be registered for the type
func (r *NodeBGP) ValidateDelete() error {
	nodebgplog.Info("validate delete", "name", r.Name)

	// TODO(user): fill in your validation logic upon object deletion.
	return nil
}

func hasOwnerReferenceByNode(nb *NodeBGP) bool {
	for _, ref := range nb.OwnerReferences {
		if ref.Kind == "Node" {
			return true
		}
	}
	return false
}
