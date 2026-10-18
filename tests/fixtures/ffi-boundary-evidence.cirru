
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |ffi-evidence
  :entries $ {} $ :default
    {} (:description "|Browser evidence fixture.") (:init-fn 'ffi-evidence.main/main!) (:mode :js) (:reload-fn 'ffi-evidence.main/reload!) (:target :browser)
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'ffi-evidence.lib $ %{} 'FileEntry
      :defs $ {} $ 'increment
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defn increment (n) (+ n 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns ffi-evidence.lib
    'ffi-evidence.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|Typed caller.")
          :code $ quote $ defn main! ()
            query-host $ js-object
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ []
        'mixed-imports $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn mixed-imports () (lib/increment 1)
            quote $ fs/existsSync |quoted
            quasiquote $ ~ $ fs/existsSync |live
            quasiquote $ ~ $ existsSync |direct
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
            :features $ #{} :js-ffi
        'quasiquoted-caller $ %{} 'CodeEntry
          :doc "|Executable unquote must remain visible to static caller evidence."
          :code $ quote $ defn quasiquoted-caller ()
            quasiquote $ do $ ~ (query-host js-object)
          :examples $ []
          :schema $ :: 'Dynamic
        'query-host $ %{} 'CodeEntry
          :doc "|Mixed browser, npm, WebGPU, and host-member evidence."
          :code $ quote $ defn query-host (host)
            do (js/document.querySelector |main) (js/navigator.gpu) (nanoid) (.-value host) (.!focus host) (.?!matches host |.active)
              quasiquote $ do $ ~ (unsafe-coerce host 'String)
              unsafe-coerce host 'String
          :examples $ []
          :ffi $ {} $ :backend :js
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'JsObject
            :features $ #{} :js-ffi
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
          :require (|nanoid :default nanoid) (|node:fs :as fs) (ffi-evidence.lib :as lib)
            |node:fs :refer $ existsSync
    'ffi-evidence.pure $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            + (lib/increment 1) (increment 2)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns ffi-evidence.pure
          :require (ffi-evidence.lib :as lib)
            ffi-evidence.lib :refer $ increment
    'ffi.helpers $ %{} 'FileEntry
      :defs $ {}
        'dependency-number $ %{} 'CodeEntry (:doc "|Dependency schema-evidence target.")
          :code $ quote $ defn dependency-number () (+ 1 2)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'typed-query $ %{} 'CodeEntry (:doc "|Exact-schema helper candidate.")
          :code $ quote $ defn typed-query (host) (unsafe-coerce host 'String)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'JsObject
            :features $ #{} :js-ffi
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns ffi.helpers
