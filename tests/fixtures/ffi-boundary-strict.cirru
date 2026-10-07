{}
  :about "|Strict FFI boundary evidence fixture."
  :package |ffi-strict
  :entries $ {} $ :default
    {}
      :description "|Browser strict workflow fixture."
      :init-fn 'ffi-strict.main/main!
      :mode :js
      :reload-fn 'ffi-strict.main/reload!
      :target :browser
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'ffi-strict.main
    %{} 'FileEntry
      :defs $ {}
        'HostNode $ %{} 'CodeEntry (:doc "|External-object trait: a trusted host handle target.")
          :code $ quote $ deftrait HostNode (:id 'String)
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object) (:target :browser)
          :schema $ :: 'Trait
        'Labelled $ %{} 'CodeEntry (:doc "|Ordinary Calcit trait: host values coerced to it still need proof.")
          :code $ quote $ deftrait Labelled (.label :fn)
          :examples $ []
          :schema $ :: 'Trait
        'as-label $ %{} 'CodeEntry (:doc "|Coercion to an ordinary trait is not a trusted host handle.")
          :code $ quote $ defn as-label (host) (unsafe-coerce host 'ffi-strict.main/Labelled)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'ffi-strict.main/Labelled)
            :args $ [] 'JsObject
            :features $ #{} :js-ffi
        'as-node $ %{} 'CodeEntry (:doc "|Coercion to an external-object trait is a retained host boundary.")
          :code $ quote $ defn as-node (host) (unsafe-coerce host 'ffi-strict.main/HostNode)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'ffi-strict.main/HostNode)
            :args $ [] 'JsObject
            :features $ #{} :js-ffi
        'main! $ %{} 'CodeEntry (:doc "|Typed caller.")
          :code $ quote $ defn main! ()
            query-host $ js-object
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ []
        'query-host $ %{} 'CodeEntry (:doc "|Browser boundary accepted by strict preprocessing.")
          :code $ quote $ defn query-host (host)
            do
              js/document.querySelector |main
              unsafe-coerce host 'String
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'JsObject
            :features $ #{} :js-ffi
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns ffi-strict.main
