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
