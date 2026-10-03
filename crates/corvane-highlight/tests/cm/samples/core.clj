;; core.clj — the main namespace of the inventory service
;;; Triple-semicolon comment with ünïcødé ✓

(ns inventory.core
  "Inventory service: tracks stock levels and reservations."
  (:require [clojure.string :as str]
            [clojure.set :refer [union difference]]
            [clojure.java.io :as io])
  (:import (java.time Instant Duration)
           [java.util UUID]))

(set! *warn-on-reflection* true)

(def ^:dynamic *default-quantity* 10)
(def ^:private registry (atom {}))
(defonce counter (atom 0))

(defrecord Item [id name quantity tags])

(defprotocol Stockable
  (restock [this n] "Adds n units.")
  (reserve [this n]))

(extend-type Item
  Stockable
  (restock [this n] (update this :quantity + n))
  (reserve [this n]
    (if (>= (:quantity this) n)
      (update this :quantity - n)
      (throw (ex-info "Not enough stock" {:item (:id this) :wanted n})))))

(defn make-item
  "Builds an item with a fresh UUID."
  ([name] (make-item name *default-quantity*))
  ([name quantity & {:keys [tags] :or {tags #{}}}]
   (->Item (UUID/randomUUID) name quantity tags)))

(defn- normalize [s]
  (-> s str/trim str/lower-case (str/replace #"\s+" "-")))

(defmacro with-timing [label & body]
  `(let [start# (System/nanoTime)
         result# (do ~@body)]
     (println ~label (/ (- (System/nanoTime) start#) 1e6) "ms")
     result#))

(defn summary [items]
  (let [total (reduce + 0 (map :quantity items))
        by-tag (group-by (comp first :tags) items)]
    {:total total
     :count (count items)
     :tags (keys by-tag)
     :ratio (/ total (max 1 (count items)))}))

(comment
  (make-item "widget")
  (summary [(make-item "a" 3) (make-item "b" 4)])
  (with-timing "restock" (restock (make-item "c") 5)))

(comment ; unbalanced parens restart on each line
  (foo (bar
  baz))
(after-comment 1 2)

(def numbers [42 -17 +3 3.14 1e10 2.5E-3 6.02e+23 22/7 0x1F 0XFF 2r1010 36rZZ 100N 1.5M 007 1.])
(def bad-numbers [1abc 12:30 0x 3.14.15])
(def chars [\a \space \newline \tab é \o177 \x41 \\ \( \" \é \😀 \abc])
(def strings ["plain" "with \"escaped\" quotes" "tab\there" "unicode: héllo 日本語"
              "multi-line
string continues
here"])
(def keywords [:simple :ns/qualified ::auto-resolved :a.b/c :true :42])
(def symbols ['quoted `syntax-quoted ~unquoted ~@spliced @deref #'var-quote ^meta sym])
(def regexes [#"[a-z]+" #"\d{3}-\d{4}" #"(?i)hello"])
(def colls {:vec [1 2 3] :set #{:a :b} :list '(1 2 3) :map {"k" "v"}, :nested {:a {:b [{:c nil}]}}})
(def specials [nil true false . new var quote recur throw try catch monitor-enter])
(def qualified [clojure.core/map java.lang.String/valueOf a.b.c/d .method Class. ..])
(def weird [a->b ->> as-> some-> *ns* *out* foo? bar! baz' quux* <=> not=])

#_(ignored form)
#?(:clj (Instant/now) :cljs (js/Date.))
#inst "2024-01-01T00:00:00.000-00:00"
#uuid "3b8a2c1e-9f5d-4c3b-8a2e-1f2d3c4b5a69"

(defn process-order
  [{:keys [id items] :as order}]
  (try
    (doseq [{:keys [sku qty]} items
            :when (pos? qty)]
      (swap! registry update sku (fnil reserve (make-item sku 0)) qty))
    (catch clojure.lang.ExceptionInfo e
      (println "failed" id (ex-data e))
      nil)
    (finally
      (swap! counter inc))))

(loop [i 0 acc []]
  (if (< i 5)
    (recur (inc i) (conj acc (* i i)))
    acc))

(let [f #(+ % %2) g (fn [x] (* x x))]
	(->> (range 10) (filter even?) (map g) (reduce f)))

(my-custom-fn 1 2)
(.toUpperCase "abc")
(Math/abs -5)
"unterminated string at the end of the file
