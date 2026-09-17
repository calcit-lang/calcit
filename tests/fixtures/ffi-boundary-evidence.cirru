{}
  :about "|FFI boundary evidence fixture."
  :package |ffi-evidence
  :entries $ {} $ :default
    {}
      :description "|Browser evidence fixture."
      :init-fn 'ffi-evidence.main/main!
      :mode :js
      :reload-fn 'ffi-evidence.main/reload!
      :target :browser
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'ffi-evidence.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|Typed caller.")
          :code $ quote $ defn main! ()
            query-host $ js-object
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ []
        'query-host $ %{} 'CodeEntry (:doc "|Mixed browser, npm, WebGPU, and host-member evidence.")
          :code $ quote $ defn query-host (host)
            do
              js/document.querySelector |main
              js/navigator.gpu
              nanoid
              .-value host
              .!focus host
              .?!matches host |.active
              quasiquote $ do
                ~ $ unsafe-coerce host 'String
              unsafe-coerce host 'String
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'JsObject
            :features $ #{} :js-ffi
        'quasiquoted-caller $ %{} 'CodeEntry (:doc "|Executable unquote must remain visible to static caller evidence.")
          :code $ quote $ defn quasiquoted-caller ()
            quasiquote $ do
              ~ $ query-host js-object
          :examples $ []
          :schema $ :: 'Dynamic
        'quoted-caller $ %{} 'CodeEntry (:doc "|Quoted data must not become caller evidence.")
          :code $ quote $ defn quoted-caller ()
            quote $ query-host js-object
          :examples $ []
          :schema $ :: 'Dynamic
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns ffi-evidence.main
          :require $ |nanoid :default nanoid
    'ffi.helpers $ %{} 'FileEntry
      :defs $ {}
        'dependency-number $ %{} 'CodeEntry (:doc "|Dependency schema-evidence target.")
          :code $ quote $ defn dependency-number ()
            + 1 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'typed-query $ %{} 'CodeEntry (:doc "|Exact-schema helper candidate.")
          :code $ quote $ defn typed-query (host)
            unsafe-coerce host 'String
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'JsObject
            :features $ #{} :js-ffi
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns ffi.helpers
