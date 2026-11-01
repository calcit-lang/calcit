
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-callback-contravariance
  :entries $ {}
    :arity $ {} (:description |)
      :init-fn 'type-fail-callback-contravariance.main/reject-wrong-arity
      :mode :native
      :reload-fn 'type-fail-callback-contravariance.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :default $ {}
      :description "|Parameter contravariance: open callbacks fit nominal callback slots, not the reverse."
      :init-fn 'type-fail-callback-contravariance.main/accept-callbacks
      :mode :native
      :reload-fn 'type-fail-callback-contravariance.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :rest-for-fixed $ {} (:description |)
      :init-fn 'type-fail-callback-contravariance.main/reject-nominal-rest-for-fixed
      :mode :native
      :reload-fn 'type-fail-callback-contravariance.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :result $ {} (:description |)
      :init-fn 'type-fail-callback-contravariance.main/reject-open-result
      :mode :native
      :reload-fn 'type-fail-callback-contravariance.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :reverse $ {} (:description |)
      :init-fn 'type-fail-callback-contravariance.main/reject-nominal-callback
      :mode :native
      :reload-fn 'type-fail-callback-contravariance.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :reverse-rest $ {} (:description |)
      :init-fn 'type-fail-callback-contravariance.main/reject-nominal-rest-callback
      :mode :native
      :reload-fn 'type-fail-callback-contravariance.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-callback-contravariance.main
    %{} 'FileEntry
      :defs $ {}
        'accept-callbacks $ %{} 'CodeEntry
          :doc "|#2499: open callbacks, fixed or rest, satisfy FsPath callback slots (parameter contravariance)."
          :code $ quote $ defn accept-callbacks () (needs-path-callback open-callback) (needs-path-rest-callback open-rest-callback) (needs-path-callback open-rest-callback)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'needs-open-callback $ %{} 'CodeEntry
          :doc "|Callback slot that may be invoked with any value."
          :code $ quote $ defn needs-open-callback (f) &unit
          :examples $ []
          :schema $ quote $ Fn
            Fn Dynamic $ -> Unit
            -> Unit
        'needs-open-rest-callback $ %{} 'CodeEntry (:doc "|Rest callback slot that may pass any values.")
          :code $ quote $ defn needs-open-rest-callback (f) &unit
          :examples $ []
          :schema $ quote $ Fn
            Fn & Dynamic $ -> Unit
            -> Unit
        'needs-path-callback $ %{} 'CodeEntry
          :doc "|Callback slot that will only be invoked with FsPath."
          :code $ quote $ defn needs-path-callback (f) &unit
          :examples $ []
          :schema $ quote $ Fn
            Fn FsPath $ -> Unit
            -> Unit
        'needs-path-rest-callback $ %{} 'CodeEntry
          :doc "|Rest callback slot invoked only with FsPath values."
          :code $ quote $ defn needs-path-rest-callback (f) &unit
          :examples $ []
          :schema $ quote $ Fn
            Fn & FsPath $ -> Unit
            -> Unit
        'needs-path-result $ %{} 'CodeEntry (:doc "|Callback slot whose result is used as FsPath.")
          :code $ quote $ defn needs-path-result (f) &unit
          :examples $ []
          :schema $ quote $ Fn
            Fn FsPath $ -> FsPath
            -> Unit
        'open-callback $ %{} 'CodeEntry
          :doc "|Accepts any value; safe wherever a narrower callback is expected."
          :code $ quote $ defn open-callback (value) &unit
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Unit)
        'open-pair-callback $ %{} 'CodeEntry (:doc "|Open parameters do not excuse a wrong arity.")
          :code $ quote $ defn open-pair-callback (a b) &unit
          :examples $ []
          :schema $ quote $ Fn Dynamic Dynamic (-> Unit)
        'open-rest-callback $ %{} 'CodeEntry (:doc "|Accepts any number of values of any type.")
          :code $ quote $ defn open-rest-callback (& values) &unit
          :examples $ []
          :schema $ quote $ Fn & Dynamic (-> Unit)
        'open-result $ %{} 'CodeEntry
          :doc "|Returns its input as Dynamic, erasing the nominal result."
          :code $ quote $ defn open-result (path) path
          :examples $ []
          :schema $ quote $ Fn FsPath (-> Dynamic)
        'path-callback $ %{} 'CodeEntry (:doc "|Accepts only FsPath.")
          :code $ quote $ defn path-callback (path) &unit
          :examples $ []
          :schema $ quote $ Fn FsPath (-> Unit)
        'path-rest-callback $ %{} 'CodeEntry (:doc "|Accepts any number of FsPath values only.")
          :code $ quote $ defn path-rest-callback (& paths) &unit
          :examples $ []
          :schema $ quote $ Fn & FsPath (-> Unit)
        'reject-nominal-callback $ %{} 'CodeEntry
          :doc "|A FsPath-only callback must not serve a slot that passes Dynamic values."
          :code $ quote $ defn reject-nominal-callback () (needs-open-callback path-callback)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reject-nominal-rest-callback $ %{} 'CodeEntry
          :doc "|A FsPath-only rest callback must not serve a rest slot that passes Dynamic values."
          :code $ quote $ defn reject-nominal-rest-callback () (needs-open-rest-callback path-rest-callback)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reject-nominal-rest-for-fixed $ %{} 'CodeEntry
          :doc "|A FsPath-only rest callback must not serve a fixed slot that passes a Dynamic value."
          :code $ quote $ defn reject-nominal-rest-for-fixed () (needs-open-callback path-rest-callback)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reject-open-result $ %{} 'CodeEntry
          :doc "|A Dynamic result must not enter a nominal callback result contract."
          :code $ quote $ defn reject-open-result () (needs-path-result open-result)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reject-wrong-arity $ %{} 'CodeEntry
          :doc "|A two-parameter callback must not fill a one-parameter slot."
          :code $ quote $ defn reject-wrong-arity () (needs-path-callback open-pair-callback)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns type-fail-callback-contravariance.main
