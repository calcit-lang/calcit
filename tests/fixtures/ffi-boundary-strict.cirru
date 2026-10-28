
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |ffi-strict
  :entries $ {} $ :default
    {} (:description "|Browser strict workflow fixture.") (:init-fn 'ffi-strict.main/main!) (:mode :js) (:reload-fn 'ffi-strict.main/reload!) (:target :browser)
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'ffi-strict.main
    %{} 'FileEntry
      :defs $ {}
        'HostNode $ %{} 'CodeEntry
          :doc "|External-object trait: a trusted host handle target."
          :code $ quote $ deftrait HostNode (:id 'String)
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object) (:target :browser)
          :schema $ quote Trait
        'Labelled $ %{} 'CodeEntry
          :doc "|Ordinary Calcit trait: host values coerced to it still need proof."
          :code $ quote $ deftrait Labelled (.label 'Fn)
          :examples $ []
          :schema $ quote Trait
        'as-label $ %{} 'CodeEntry
          :doc "|Coercion to an ordinary trait is not a trusted host handle."
          :code $ quote $ defn as-label (host) (unsafe-coerce host 'ffi-strict.main/Labelled)
          :examples $ []
          :schema $ quote $ Fn JsObject (-> ffi-strict.main/Labelled) (:features :js-ffi)
        'as-node $ %{} 'CodeEntry
          :doc "|Coercion to an external-object trait is a retained host boundary."
          :code $ quote $ defn as-node (host) (unsafe-coerce host 'ffi-strict.main/HostNode)
          :examples $ []
          :schema $ quote $ Fn JsObject (-> ffi-strict.main/HostNode) (:features :js-ffi)
        'main! $ %{} 'CodeEntry (:doc "|Typed caller.")
          :code $ quote $ defn main! ()
            query-host $ js-object
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'query-host $ %{} 'CodeEntry
          :doc "|Browser boundary accepted by strict preprocessing."
          :code $ quote $ defn query-host (host)
            do (js/document.querySelector |main) (unsafe-coerce host 'String)
          :examples $ []
          :schema $ quote $ Fn JsObject (-> String) (:features :js-ffi)
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns ffi-strict.main
