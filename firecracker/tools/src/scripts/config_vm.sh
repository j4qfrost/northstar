#!/bin/sh
#
# Config the VM based on the input json file
#

#set -xv
vsock_server="/bin/nc-vsock"
vsock_srvport=2
vsock_cfgport=0

if [ $# -ne 1 ] ; then
	echo "Usage: $0 output_file"
	exit 1
fi
filename=$1


get_timestamp() {
        local now tmp usec

        now=$(date +"%Y-%m-%dT%T")
        tmp=$(adjtimex | grep tv_usec)
        usec=${tmp##*: }
        echo ${now}.${usec}
}

logit() {
	local str="$@" now

	now=$(get_timestamp)
	echo "$now: $str"
}

wait_for_file() {
	local outfile=$1 port

	$vsock_server $vsock_srvport $vsock_cfgport > $outfile
	return $?
}

# Extract a key from a json file
grok() {
	local file=$1 key=$2 val

	val=$(jq -r "$key" $file)
	if [ $? -ne 0 ] ; then
		return 1
	fi
	echo $val
	return 0
}

ifup() {
	local file=$1 
	local ipaddr_cidr gw

	logit "start network config"

	ipaddr_cidr=$(grok $file ".netconf.ipaddr + \"/\" + .netconf.cidr")
	if [ $? -ne 0 ] ; then
		echo "can not get ipaddr param"
		exit 1
	fi

	gw=$(grok $file ".netconf.gateway")
	if [ $? -ne 0 ] ; then
		echo "no gateway address param"
		exit 1
	fi

	/sbin/ip addr add $ipaddr_cidr dev eth0
	if [ $? -ne 0 ] ; then
		echo "Can not configure eth0"
		exit 1
	fi

	/sbin/ip link set eth0 up
	if [ $? -ne 0 ] ; then
		echo "Can not bring up eth0"
		exit 1
	fi

	/sbin/ip route add default via $gw dev eth0
	if [ $? -ne 0 ] ; then
		echo "Can not configure default route"
		exit 1
	fi
	
	logit "end network config"
}

do_mounts() {
	local file=$1 status cmd
	local i linecnt 

	logit "start mount config"
	linecnt=$(jq -r -c '.mounts[]' $file | wc -l)
	status=$?
	if [ $status -ne 0 ] ; then
		echo "Can not get number of mountpoints, status $status"
		exit 1
	fi
	
	# The root is already mounted by firecracker, so we skip it
	i=1
	while [[ $i -lt $linecnt ]] ; do
		cmd=$(grok $file ".mounts[$i] | \
			.flags + \" \" + .dev  + \" \" + .mountpoint")
		status=$?
		if [ $status -ne 0 ] ; then
			echo "Can not get device entry $i, status $status"
			exit 1
		fi
		logit "mount $cmd"
		mount $cmd 
		status=$?
		if [ $status -ne 0 ] ; then
			echo "Can not mount $dev at $mountpoint, status $status"
			exit 1
		fi
		let i=i+1
	done

	logit "end mount config"
}
	
#
# Simple sanity check on file
verify_file() {
	local file=$1 status outfile errfile

	outfile=/tmp/out_$$.txt
	errfile=/tmp/err_$$.txt

	jq -r '.' $file > $outfile 2>$errfile
	status=$?
	if [ $status -ne 0 ] ; then
		echo "$file is invalid json, status $status. jq reports the following"
		cat $errfile
	fi
	rm $outfile
	rm $errfile

	return $status
}

logit "start VM config"

wait_for_file $filename
if [ $? -ne 0 ] ; then
	echo "Can not receive file $filename"
	exit 1
fi

verify_file $filename
if [ $? -ne 0 ] ; then
	echo "Can not validate json config file"
	exit 1
fi

# Config the network from what we received
ifup $filename

# Mount the optional filesystems
do_mounts $filename

logit "End VM config"

exit 0

